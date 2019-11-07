use super::WORLD_SIZE;
use bitvec::prelude::{bitbox, bitvec, BitBox};
use common::{terrain::TerrainChunkSize, vol::RectVolSize};
use num::Float;
use rayon::prelude::*;
use std::{f32, f64, u32};
use vek::*;

/// Calculates the smallest distance along an axis (x, y) from an edge of
/// the world.  This value is maximal at WORLD_SIZE / 2 and minimized at the extremes
/// (0 or WORLD_SIZE on one or more axes).  It then divides the quantity by cell_size,
/// so the final result is 1 when we are not in a cell along the edge of the world, and
/// ranges between 0 and 1 otherwise (lower when the chunk is closer to the edge).
pub fn map_edge_factor(posi: usize) -> f32 {
    uniform_idx_as_vec2(posi)
        .map2(WORLD_SIZE.map(|e| e as i32), |e, sz| {
            (sz / 2 - (e - sz / 2).abs()) as f32 / 16.0
        })
        .reduce_partial_min()
        .max(0.0)
        .min(1.0)
}

/// Computes the cumulative distribution function of the weighted sum of k independent,
/// uniformly distributed random variables between 0 and 1.  For each variable i, we use weights[i]
/// as the weight to give samples[i] (the weights should all be positive).
///
/// If the precondition is met, the distribution of the result of calling this function will be
/// uniformly distributed while preserving the same information that was in the original average.
///
/// For N > 33 the function will no longer return correct results since we will overflow u32.
///
/// NOTE:
///
/// Per [1], the problem of determing the CDF of
/// the sum of uniformly distributed random variables over *different* ranges is considerably more
/// complicated than it is for the same-range case.  Fortunately, it also provides a reference to
/// [2], which contains a complete derivation of an exact rule for the density function for
/// this case.  The CDF is just the integral of the cumulative distribution function [3],
/// which we use to convert this into a CDF formula.
///
/// This allows us to sum weighted, uniform, independent random variables.
///
/// At some point, we should probably contribute this back to stats-rs.
///
/// 1. https://www.r-bloggers.com/sums-of-random-variables/,
/// 2. Sadooghi-Alvandi, S., A. Nematollahi, & R. Habibi, 2009.
///    On the Distribution of the Sum of Independent Uniform Random Variables.
///    Statistical Papers, 50, 171-175.
/// 3. hhttps://en.wikipedia.org/wiki/Cumulative_distribution_function
pub fn cdf_irwin_hall<const N: usize>(weights: &[f32; N], samples: [f32; N]) -> f32 {
    // Let J_k = {(j_1, ... , j_k) : 1 ≤ j_1 < j_2 < ··· < j_k ≤ N }.
    //
    // Let A_N = Π{k = 1 to n}a_k.
    //
    // The density function for N ≥ 2 is:
    //
    //   1/(A_N * (N - 1)!) * (x^(N-1) + Σ{k = 1 to N}((-1)^k *
    //   Σ{(j_1, ..., j_k) ∈ J_k}(max(0, x - Σ{l = 1 to k}(a_(j_l)))^(N - 1))))
    //
    // So the cumulative distribution function is its integral, i.e. (I think)
    //
    // 1/(product{k in A}(k) * N!) * (x^N + sum(k in 1 to N)((-1)^k *
    // sum{j in Subsets[A, {k}]}(max(0, x - sum{l in j}(l))^N)))
    //
    // which is also equivalent to
    //
    //   (letting B_k = { a in Subsets[A, {k}] : sum {l in a} l }, B_(0,1) = 0 and
    //            H_k = { i : 1 ≤ 1 ≤ N! / (k! * (N - k)!) })
    //
    //   1/(product{k in A}(k) * N!) * sum(k in 0 to N)((-1)^k *
    //   sum{l in H_k}(max(0, x - B_(k,l))^N))
    //
    // We should be able to iterate through the whole power set
    // instead, and figure out K by calling count_ones(), so we can compute the result in O(2^N)
    // iterations.
    let x: f64 = weights
        .iter()
        .zip(samples.iter())
        .map(|(&weight, &sample)| weight as f64 * sample as f64)
        .sum();

    let mut y = 0.0f64;
    for subset in 0u32..(1 << N) {
        // Number of set elements
        let k = subset.count_ones();
        // Add together exactly the set elements to get B_subset
        let z = weights
            .iter()
            .enumerate()
            .filter(|(i, _)| subset & (1 << i) as u32 != 0)
            .map(|(_, &k)| k as f64)
            .sum::<f64>();
        // Compute max(0, x - B_subset)^N
        let z = (x - z).max(0.0).powi(N as i32);
        // The parity of k determines whether the sum is negated.
        y += if k & 1 == 0 { z } else { -z };
    }

    // Divide by the product of the weights.
    y /= weights.iter().map(|&k| k as f64).product::<f64>();

    // Remember to multiply by 1 / N! at the end.
    (y / (1..(N as i32) + 1).product::<i32>() as f64) as f32
}

/// First component of each element of the vector is the computed CDF of the noise function at this
/// index (i.e. its position in a sorted list of value returned by the noise function applied to
/// every chunk in the game).  Second component is the cached value of the noise function that
/// generated the index.
///
/// NOTE: Length should always be WORLD_SIZE.x * WORLD_SIZE.y.
pub type InverseCdf<F = f32> = Box<[(f32, F)]>;

/// Computes the position Vec2 of a SimChunk from an index, where the index was generated by
/// uniform_noise.
pub fn uniform_idx_as_vec2(idx: usize) -> Vec2<i32> {
    Vec2::new((idx % WORLD_SIZE.x) as i32, (idx / WORLD_SIZE.x) as i32)
}

/// Computes the index of a Vec2 of a SimChunk from a position, where the index is generated by
/// uniform_noise.  NOTE: Both components of idx should be in-bounds!
pub fn vec2_as_uniform_idx(idx: Vec2<i32>) -> usize {
    (idx.y as usize * WORLD_SIZE.x + idx.x as usize) as usize
}

/// Compute inverse cumulative distribution function for arbitrary function f, the hard way.  We
/// pre-generate noise values prior to worldgen, then sort them in order to determine the correct
/// position in the sorted order.  That lets us use `(index + 1) / (WORLDSIZE.y * WORLDSIZE.x)` as
/// a uniformly distributed (from almost-0 to 1) regularization of the chunks.  That is, if we
/// apply the computed "function" F⁻¹(x, y) to (x, y) and get out p, it means that approximately
/// (100 * p)% of chunks have a lower value for F⁻¹ than p.  The main purpose of doing this is to
/// make sure we are using the entire range we want, and to allow us to apply the numerous results
/// about distributions on uniform functions to the procedural noise we generate, which lets us
/// much more reliably control the *number* of features in the world while still letting us play
/// with the *shape* of those features, without having arbitrary cutoff points / discontinuities
/// (which tend to produce ugly-looking / unnatural terrain).
///
/// As a concrete example, before doing this it was very hard to tweak humidity so that either most
/// of the world wasn't dry, or most of it wasn't wet, by combining the billow noise function and
/// the computed altitude.  This is because the billow noise function has a very unusual
/// distribution that is heavily skewed towards 0.  By correcting for this tendency, we can start
/// with uniformly distributed billow noise and altitudes and combine them to get uniformly
/// distributed humidity, while still preserving the existing shapes that the billow noise and
/// altitude functions produce.
///
/// f takes an index, which represents the index corresponding to this chunk in any any SimChunk
/// vector returned by uniform_noise, and (for convenience) the float-translated version of those
/// coordinates.
/// f should return a value with no NaNs.  If there is a NaN, it will panic.  There are no other
/// conditions on f.  If f returns None, the value will be set to NaN, and will be ignored for the
/// purposes of computing the uniform range.
///
/// Returns a vec of (f32, f32) pairs consisting of the percentage of chunks with a value lower than
/// this one, and the actual noise value (we don't need to cache it, but it makes ensuring that
/// subsequent code that needs the noise value actually uses the same one we were using here
/// easier).  Also returns the "inverted index" pointing from a position to a noise.
pub fn uniform_noise<F: Float + Send>(
    f: impl Fn(usize, Vec2<f64>) -> Option<F> + Sync,
) -> (InverseCdf<F>, Box<[(usize, F)]>) {
    let mut noise = (0..WORLD_SIZE.x * WORLD_SIZE.y)
        .into_par_iter()
        .filter_map(|i| {
            (f(
                i,
                (uniform_idx_as_vec2(i) * TerrainChunkSize::RECT_SIZE.map(|e| e as i32))
                    .map(|e| e as f64),
            )
            .map(|res| (i, res)))
        })
        .collect::<Vec<_>>();

    // sort_unstable_by is equivalent to sort_by here since we include a unique index in the
    // comparison.  We could leave out the index, but this might make the order not
    // reproduce the same way between different versions of Rust (for example).
    noise.par_sort_unstable_by(|f, g| (f.1, f.0).partial_cmp(&(g.1, g.0)).unwrap());

    // Construct a vector that associates each chunk position with the 1-indexed
    // position of the noise in the sorted vector (divided by the vector length).
    // This guarantees a uniform distribution among the samples (excluding those that returned
    // None, which will remain at zero).
    let mut uniform_noise = vec![(0.0, F::nan()/*zero()*/); WORLD_SIZE.x * WORLD_SIZE.y].into_boxed_slice();
    // NOTE: Consider using try_into here and elsewhere in this function, since i32::MAX
    // technically doesn't fit in an f32 (even if we should never reach that limit).
    let total = noise.len() as f32;
    for (noise_idx, &(chunk_idx, noise_val)) in noise.iter().enumerate() {
        uniform_noise[chunk_idx] = ((1 + noise_idx) as f32 / total, noise_val);
    }
    (uniform_noise, noise.into_boxed_slice())
}

/// Iterate through all cells adjacent and including four chunks whose top-left point is posi.
/// This isn't just the immediate neighbors of a chunk plus the center, because it is designed
/// to cover neighbors of a point in the chunk's "interior."
///
/// This is what's used during cubic interpolation, for example, as it guarantees that for any
/// point between the given chunk (on the top left) and its top-right/down-right/down neighbors,
/// the twelve chunks surrounding this box (its "perimeter") are also inspected.
pub fn local_cells(posi: usize) -> impl Clone + Iterator<Item = usize> {
    let pos = uniform_idx_as_vec2(posi);
    // NOTE: want to keep this such that the chunk index is in ascending order!
    let grid_size = 3i32;
    let grid_bounds = 2 * grid_size + 1;
    (0..grid_bounds * grid_bounds)
        .into_iter()
        .map(move |index| {
            Vec2::new(
                pos.x + (index % grid_bounds) - grid_size,
                pos.y + (index / grid_bounds) - grid_size,
            )
        })
        .filter(|pos| {
            pos.x >= 0 && pos.y >= 0 && pos.x < WORLD_SIZE.x as i32 && pos.y < WORLD_SIZE.y as i32
        })
        .map(vec2_as_uniform_idx)
}

/// Iterate through all cells adjacent to a chunk.
pub fn neighbors(posi: usize) -> impl Clone + Iterator<Item = usize> {
    let pos = uniform_idx_as_vec2(posi);
    // NOTE: want to keep this such that the chunk index is in ascending order!
    [
        (-1, -1),
        (0, -1),
        (1, -1),
        (-1, 0),
        (1, 0),
        (-1, 1),
        (0, 1),
        (1, 1),
    ]
    .iter()
    .map(move |&(x, y)| Vec2::new(pos.x + x, pos.y + y))
    .filter(|pos| {
        pos.x >= 0 && pos.y >= 0 && pos.x < WORLD_SIZE.x as i32 && pos.y < WORLD_SIZE.y as i32
    })
    .map(vec2_as_uniform_idx)
}

// Note that we should already have okay cache locality since we have a grid.
pub fn uphill<'a>(dh: &'a [isize], posi: usize) -> impl Clone + Iterator<Item = usize> + 'a {
    neighbors(posi).filter(move |&posj| dh[posj] == posi as isize)
}

/// Compute the neighbor "most downhill" from all chunks.
///
/// TODO: See if allocating in advance is worthwhile.
pub fn downhill(h: &[f32], is_ocean: impl Fn(usize) -> bool + Sync) -> Box<[isize]> {
    // Constructs not only the list of downhill nodes, but also computes an ordering (visiting
    // nodes in order from roots to leaves).
    h.par_iter()
        .enumerate()
        .map(|(posi, &nh)| {
            let _pos = uniform_idx_as_vec2(posi);
            if is_ocean(posi) {
                -2
            } else {
                let mut best = -1;
                let mut besth = nh;
                for nposi in neighbors(posi) {
                    let nbh = h[nposi];
                    if nbh < besth {
                        besth = nbh;
                        best = nposi as isize;
                    }
                }
                best
            }
        })
        .collect::<Vec<_>>()
        .into_boxed_slice()
}

/// Find all ocean tiles from a height map, using an inductive definition of ocean as one of:
/// - posi is at the side of the world (map_edge_factor(posi) == 0.0)
/// - posi has a neighboring ocean tile, and has a height below sea level (oldh(posi) <= 0.0).
pub fn get_oceans(oldh: impl Fn(usize) -> f32 + Sync) -> BitBox {
    // We can mark tiles as ocean candidates by scanning row by row, since the top edge is ocean,
    // the sides are connected to it, and any subsequent ocean tiles must be connected to it.
    let mut is_ocean = bitbox![0; WORLD_SIZE.x * WORLD_SIZE.y];
    let mut stack = Vec::new();
    for x in 0..WORLD_SIZE.x as i32 {
        stack.push(vec2_as_uniform_idx(Vec2::new(x, 0)));
        stack.push(vec2_as_uniform_idx(Vec2::new(x, WORLD_SIZE.y as i32 - 1)));
    }
    for y in 1..WORLD_SIZE.y as i32 - 1 {
        stack.push(vec2_as_uniform_idx(Vec2::new(0, y)));
        stack.push(vec2_as_uniform_idx(Vec2::new(WORLD_SIZE.x as i32 - 1, y)));
    }
    while let Some(chunk_idx) = stack.pop() {
        // println!("Ocean chunk {:?}: {:?}", uniform_idx_as_vec2(chunk_idx), oldh(chunk_idx));
        if *is_ocean.at(chunk_idx) {
            continue;
        }
        *is_ocean.at(chunk_idx) = true;
        stack.extend(neighbors(chunk_idx).filter(|&neighbor_idx| {
            // println!("Ocean neighbor: {:?}: {:?}", uniform_idx_as_vec2(neighbor_idx), oldh(neighbor_idx));
            oldh(neighbor_idx) <= 0.0
        }));
    }
    is_ocean
}
