//! Palette production. Ports `NeuralQuantizer.cs` (NeuQuant) and `OctreeQuantizer.cs`.
//!
//! Both ported from ScreenToGif's C# which is itself the NeuQuant reference; the
//! arithmetic is kept bit-for-bit so palettes match the C# build.

use crate::Rgb;

const BYTES_PER_PIXEL: usize = 4;

pub trait Quantizer {
    /// Scans pixel data and fills internal state.
    fn first_pass(&mut self, bgra: &[u8]);
    /// Produces the palette from the scan result.
    fn build_palette(&mut self) -> Vec<Rgb>;
    /// Scan + palette in one call.
    fn quantize(&mut self, bgra: &[u8]) -> Vec<Rgb> {
        self.first_pass(bgra);
        self.build_palette()
    }
}

// ─── NeuQuant ─────────────────────────────────────────────────────────────────

const NETWORK_BIAS_SHIFT: i32 = 4;
const NUMBER_OF_LEARNING_CYCLES: usize = 100;

const PRIME1: usize = 499;
const PRIME2: usize = 491;
const PRIME3: usize = 487;
const PRIME4: usize = 503;

const ALPHA_BIAS_SHIFT: i32 = 10;
const INITIAL_ALPHA: i32 = 1 << ALPHA_BIAS_SHIFT;
const INT_BIAS_SHIFT: i32 = 16;
const INT_BIAS: i32 = 1 << INT_BIAS_SHIFT;
const GAMMA_SHIFT: i32 = 10;
const BETA_SHIFT: i32 = 10;
const CLOSEST_NEURON_FREQUENCY_INCREMENT: i32 = INT_BIAS >> BETA_SHIFT;
const CLOSEST_NEURON_BIAS_DECREMENT: i32 = INT_BIAS << (GAMMA_SHIFT - BETA_SHIFT);

const NEIGHBOURHOOD_SIZE_BIAS_SHIFT: i32 = 6;
const NEIGHBOURHOOD_SIZE_BIAS: i32 = 1 << NEIGHBOURHOOD_SIZE_BIAS_SHIFT;
const UNBIASED_NEIGHBOURHOOD_SIZE_DECREMENT: i32 = 30;

const RADIUS_BIAS_SHIFT: i32 = 8;
const RADIUS_BIAS: i32 = 1 << RADIUS_BIAS_SHIFT;
const ALPHA_RADIUS_BIAS: i32 = 1 << (ALPHA_BIAS_SHIFT + RADIUS_BIAS_SHIFT);

/// Self-organizing neural-network color quantization. Slower than Octree, but
/// produces visibly better palettes on photographic content.
pub struct NeuralQuantizer {
    network_size: usize,
    sampling_factor: i32,
    network: Vec<[i32; 4]>,
    biases: Vec<i32>,
    frequencies: Vec<i32>,
    neighbourhood_alphas: Vec<i32>,
    initial_unbiased_neighbourhood_size: i32,
}

impl NeuralQuantizer {
    /// `sampling_factor` 1..=20 (1 = best quality, slowest); `maximum_colors` 2..=256.
    pub fn new(sampling_factor: i32, maximum_colors: usize) -> Self {
        let network_size = maximum_colors.clamp(2, 256);
        Self {
            network_size,
            sampling_factor: sampling_factor.clamp(1, 20),
            network: Vec::new(),
            biases: Vec::new(),
            frequencies: Vec::new(),
            neighbourhood_alphas: Vec::new(),
            initial_unbiased_neighbourhood_size: 0,
        }
    }

    fn learn(&mut self, pixels: &[u8]) {
        let byte_count = pixels.len();
        if byte_count < BYTES_PER_PIXEL {
            return;
        }

        if byte_count < PRIME4 * BYTES_PER_PIXEL {
            self.sampling_factor = 1;
        }

        let alpha_decrement = 30 + (self.sampling_factor - 1) / 4;
        let pixels_to_examine = (byte_count / (BYTES_PER_PIXEL * self.sampling_factor as usize)).max(1);
        let alpha_update_frequency = (pixels_to_examine / NUMBER_OF_LEARNING_CYCLES).max(1);
        let mut alpha = INITIAL_ALPHA;
        let mut unbiased_size = self.initial_unbiased_neighbourhood_size;
        let mut neighbourhood_size = (unbiased_size >> NEIGHBOURHOOD_SIZE_BIAS_SHIFT).max(1);

        self.set_neighbourhood_alphas(neighbourhood_size, alpha);

        let step = get_pixel_index_increment(byte_count);
        let mut pixel_index = 0usize;

        for examined in 0..pixels_to_examine {
            if pixel_index + 3 < byte_count && pixels[pixel_index + 3] > 0 {
                let b = (pixels[pixel_index] as i32) << NETWORK_BIAS_SHIFT;
                let g = (pixels[pixel_index + 1] as i32) << NETWORK_BIAS_SHIFT;
                let r = (pixels[pixel_index + 2] as i32) << NETWORK_BIAS_SHIFT;

                let best = self.find_best_neuron(b, g, r);
                self.move_neuron(alpha, best, b, g, r);
                if neighbourhood_size != 0 {
                    self.move_neighbouring_neurons(neighbourhood_size, best, b, g, r);
                }
            }

            pixel_index += step;
            if pixel_index >= byte_count {
                pixel_index -= byte_count;
            }

            if examined % alpha_update_frequency != 0 {
                continue;
            }

            alpha -= alpha / alpha_decrement;
            unbiased_size -= unbiased_size / UNBIASED_NEIGHBOURHOOD_SIZE_DECREMENT;
            neighbourhood_size = unbiased_size >> NEIGHBOURHOOD_SIZE_BIAS_SHIFT;
            if neighbourhood_size <= 1 {
                neighbourhood_size = 0;
            }
            self.set_neighbourhood_alphas(neighbourhood_size, alpha);
        }
    }

    fn set_neighbourhood_alphas(&mut self, size: i32, alpha: i32) {
        if size <= 0 {
            return;
        }

        let size = (size as usize).min(self.neighbourhood_alphas.len());
        let squared = (size * size) as i32;
        if squared == 0 {
            return;
        }
        for i in 0..size {
            self.neighbourhood_alphas[i] = alpha * ((squared - (i * i) as i32) * RADIUS_BIAS / squared);
        }
    }

    fn find_best_neuron(&mut self, blue: i32, green: i32, red: i32) -> usize {
        let mut best_distance = i32::MAX;
        let mut best_bias_distance = i32::MAX;
        let mut closest_index = 0usize;
        let mut best_bias_index = 0usize;

        for n in 0..self.network.len() {
            let neuron = &self.network[n];

            let distance = (neuron[0] - blue).abs() + (neuron[1] - green).abs() + (neuron[2] - red).abs();

            if distance < best_distance {
                best_distance = distance;
                closest_index = n;
            }

            let bias_distance = distance - (self.biases[n] >> (INT_BIAS_SHIFT - NETWORK_BIAS_SHIFT));
            if bias_distance < best_bias_distance {
                best_bias_distance = bias_distance;
                best_bias_index = n;
            }

            let frequency_delta = self.frequencies[n] >> BETA_SHIFT;
            self.frequencies[n] -= frequency_delta;
            self.biases[n] += frequency_delta << GAMMA_SHIFT;
        }

        self.frequencies[closest_index] += CLOSEST_NEURON_FREQUENCY_INCREMENT;
        self.biases[closest_index] -= CLOSEST_NEURON_BIAS_DECREMENT;
        best_bias_index
    }

    fn move_neuron(&mut self, alpha: i32, index: usize, b: i32, g: i32, r: i32) {
        let neuron = &mut self.network[index];
        neuron[0] -= alpha * (neuron[0] - b) / INITIAL_ALPHA;
        neuron[1] -= alpha * (neuron[1] - g) / INITIAL_ALPHA;
        neuron[2] -= alpha * (neuron[2] - r) / INITIAL_ALPHA;
    }

    fn move_neighbouring_neurons(&mut self, size: i32, index: usize, b: i32, g: i32, r: i32) {
        let low = (index as i32 - size).max(-1);
        let high = ((index as i32 + size) as usize).min(self.network.len());
        let high = high as i32;

        let mut high_index = index as i32 + 1;
        let mut low_index = index as i32 - 1;
        let mut alpha_index = 1usize;

        while (high_index < high || low_index > low)
            && alpha_index < self.neighbourhood_alphas.len()
        {
            let alpha = self.neighbourhood_alphas[alpha_index];
            alpha_index += 1;
            if high_index < high {
                self.move_neighbour(high_index as usize, alpha, b, g, r);
                high_index += 1;
            }
            if low_index > low {
                self.move_neighbour(low_index as usize, alpha, b, g, r);
                low_index -= 1;
            }
        }
    }

    fn move_neighbour(&mut self, index: usize, alpha: i32, b: i32, g: i32, r: i32) {
        let neuron = &mut self.network[index];
        neuron[0] -= alpha * (neuron[0] - b) / ALPHA_RADIUS_BIAS;
        neuron[1] -= alpha * (neuron[1] - g) / ALPHA_RADIUS_BIAS;
        neuron[2] -= alpha * (neuron[2] - r) / ALPHA_RADIUS_BIAS;
    }

    fn unbias_network(&mut self) {
        for neuron in &mut self.network {
            neuron[0] = (neuron[0] >> NETWORK_BIAS_SHIFT).clamp(0, 255);
            neuron[1] = (neuron[1] >> NETWORK_BIAS_SHIFT).clamp(0, 255);
            neuron[2] = (neuron[2] >> NETWORK_BIAS_SHIFT).clamp(0, 255);
        }
    }
}

impl Quantizer for NeuralQuantizer {
    fn first_pass(&mut self, bgra: &[u8]) {
        let size = self.network_size.clamp(2, 256);

        self.network = vec![[0i32; 4]; size];
        self.biases = vec![0; size];
        self.frequencies = vec![0; size];

        let initial_neighbourhood = (size >> 3).max(1);
        self.neighbourhood_alphas = vec![0; initial_neighbourhood];
        self.initial_unbiased_neighbourhood_size = initial_neighbourhood as i32 * NEIGHBOURHOOD_SIZE_BIAS;

        for n in 0..size {
            let v = ((n as i32) << (NETWORK_BIAS_SHIFT + 8)) / size as i32;
            self.network[n] = [v, v, v, 0];
            self.frequencies[n] = INT_BIAS / size as i32;
        }

        self.learn(bgra);
        self.unbias_network();
    }

    fn build_palette(&mut self) -> Vec<Rgb> {
        self.network
            .iter()
            .map(|n| {
                Rgb::new(
                    n[2].clamp(0, 255) as u8,
                    n[1].clamp(0, 255) as u8,
                    n[0].clamp(0, 255) as u8,
                )
            })
            .collect()
    }
}

fn get_pixel_index_increment(byte_count: usize) -> usize {
    if byte_count < PRIME4 * BYTES_PER_PIXEL {
        return BYTES_PER_PIXEL;
    }
    if byte_count % PRIME1 != 0 {
        return PRIME1 * BYTES_PER_PIXEL;
    }
    if byte_count % PRIME2 != 0 {
        return PRIME2 * BYTES_PER_PIXEL;
    }
    if byte_count % PRIME3 != 0 {
        return PRIME3 * BYTES_PER_PIXEL;
    }
    PRIME4 * BYTES_PER_PIXEL
}

// ─── Octree ───────────────────────────────────────────────────────────────────

const COLOR_BITS: usize = 8;

/// Fast, low-memory octree quantization.
pub struct OctreeQuantizer {
    octree: Octree,
    max_colors: usize,
}

impl OctreeQuantizer {
    pub fn new(max_colors: usize) -> Self {
        Self {
            octree: Octree::new(COLOR_BITS),
            max_colors,
        }
    }
}

impl Quantizer for OctreeQuantizer {
    fn first_pass(&mut self, bgra: &[u8]) {
        let mut i = 0;
        while i + 3 < bgra.len() {
            if bgra[i + 3] != 0 {
                self.octree.add_color(bgra[i + 2], bgra[i + 1], bgra[i]);
            }
            i += BYTES_PER_PIXEL;
        }
    }

    fn build_palette(&mut self) -> Vec<Rgb> {
        self.octree.palletize(self.max_colors.clamp(2, 256))
    }
}

struct OctreeNode {
    children: [Option<usize>; 8],
    is_leaf: bool,
    pixel_count: i64,
    red: i64,
    green: i64,
    blue: i64,
    next_reducible: Option<usize>,
    #[allow(dead_code)]
    palette_index: usize,
}

impl OctreeNode {
    fn branch(next_reducible: Option<usize>) -> Self {
        Self {
            children: [None; 8],
            is_leaf: false,
            pixel_count: 0,
            red: 0,
            green: 0,
            blue: 0,
            next_reducible,
            palette_index: 0,
        }
    }

    fn leaf() -> Self {
        Self {
            children: [None; 8],
            is_leaf: true,
            pixel_count: 0,
            red: 0,
            green: 0,
            blue: 0,
            next_reducible: None,
            palette_index: 0,
        }
    }
}

struct Octree {
    nodes: Vec<OctreeNode>,
    reducible_nodes: [Option<usize>; COLOR_BITS + 1],
    previous_node: Option<usize>,
    previous_color: i32,
    leaves: usize,
    max_color_bits: usize,
}

/// C# masks shift counts to 5 bits (`x >> -2` becomes `x >> 30`). The original
/// octree relies on that at the deepest levels; replicated here so produced
/// palettes stay byte-identical to the C# build.
#[inline]
fn shift_masked(value: i32, count: i32) -> i32 {
    value >> (count & 31)
}

impl Octree {
    fn new(max_color_bits: usize) -> Self {
        let root = if max_color_bits == 0 {
            OctreeNode::leaf()
        } else {
            OctreeNode::branch(None)
        };
        let mut tree = Self {
            nodes: vec![root],
            reducible_nodes: [None; COLOR_BITS + 1],
            previous_node: None,
            previous_color: -1,
            leaves: 0,
            max_color_bits,
        };
        if max_color_bits == 0 {
            tree.leaves = 1;
        } else {
            tree.reducible_nodes[0] = Some(0);
        }
        tree
    }

    fn new_node(&mut self, level: usize) -> usize {
        let index = self.nodes.len();
        if level == self.max_color_bits {
            self.leaves += 1;
            self.nodes.push(OctreeNode::leaf());
        } else {
            let next = self.reducible_nodes[level];
            self.reducible_nodes[level] = Some(index);
            self.nodes.push(OctreeNode::branch(next));
        }
        index
    }

    fn add_color(&mut self, r: u8, g: u8, b: u8) {
        let packed = ((r as i32) << 16) | ((g as i32) << 8) | b as i32;
        if packed == self.previous_color {
            if let Some(node) = self.previous_node {
                self.increment(node, r, g, b);
                return;
            }
        }

        self.previous_color = packed;
        self.add_color_at(0, r, g, b, 0);
    }

    fn add_color_at(&mut self, index: usize, r: u8, g: u8, b: u8, level: usize) {
        if self.nodes[index].is_leaf {
            self.increment(index, r, g, b);
            self.previous_node = Some(index);
            return;
        }

        let child_slot = child_index(r, g, b, level);
        let child = match self.nodes[index].children[child_slot] {
            Some(c) => c,
            None => {
                let c = self.new_node(level + 1);
                self.nodes[index].children[child_slot] = Some(c);
                c
            }
        };
        self.add_color_at(child, r, g, b, level + 1);
    }

    fn increment(&mut self, index: usize, r: u8, g: u8, b: u8) {
        let node = &mut self.nodes[index];
        node.pixel_count += 1;
        node.red += r as i64;
        node.green += g as i64;
        node.blue += b as i64;
    }

    fn palletize(&mut self, color_count: usize) -> Vec<Rgb> {
        while self.leaves > color_count {
            if !self.reduce() {
                break;
            }
        }

        let mut palette = Vec::with_capacity(self.leaves);
        let mut index = 0usize;
        self.construct_palette(0, &mut palette, &mut index);
        palette
    }

    /// Returns false when there is nothing left to reduce.
    fn reduce(&mut self) -> bool {
        let mut level = self.max_color_bits.saturating_sub(1);
        while level > 0 && self.reducible_nodes[level].is_none() {
            level -= 1;
        }

        let Some(node) = self.reducible_nodes[level] else {
            return false;
        };

        self.reducible_nodes[level] = self.nodes[node].next_reducible;
        let removed = self.reduce_node(node);
        self.leaves -= removed;
        self.previous_node = None;
        self.previous_color = -1;
        true
    }

    fn reduce_node(&mut self, index: usize) -> usize {
        let mut red = 0i64;
        let mut green = 0i64;
        let mut blue = 0i64;
        let mut pixel_count = 0i64;
        let mut children = 0usize;

        for i in 0..8 {
            if let Some(child) = self.nodes[index].children[i] {
                red += self.nodes[child].red;
                green += self.nodes[child].green;
                blue += self.nodes[child].blue;
                pixel_count += self.nodes[child].pixel_count;
                children += 1;
                self.nodes[index].children[i] = None;
            }
        }

        let node = &mut self.nodes[index];
        node.red = red;
        node.green = green;
        node.blue = blue;
        node.pixel_count = pixel_count;
        node.is_leaf = true;
        children.saturating_sub(1)
    }

    fn construct_palette(&mut self, index: usize, palette: &mut Vec<Rgb>, counter: &mut usize) {
        if self.nodes[index].is_leaf {
            self.nodes[index].palette_index = *counter;
            *counter += 1;
            let node = &self.nodes[index];
            let count = node.pixel_count.max(1);
            palette.push(Rgb::new(
                (node.red / count) as u8,
                (node.green / count) as u8,
                (node.blue / count) as u8,
            ));
            return;
        }

        for i in 0..8 {
            if let Some(child) = self.nodes[index].children[i] {
                self.construct_palette(child, palette, counter);
            }
        }
    }
}

fn child_index(r: u8, g: u8, b: u8, level: usize) -> usize {
    const MASK: [i32; 8] = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];
    let shift = 7 - level as i32;
    let ri = shift_masked(r as i32 & MASK[level], shift - 2);
    let gi = shift_masked(g as i32 & MASK[level], shift - 1);
    let bi = shift_masked(b as i32 & MASK[level], shift);
    ((ri | gi | bi) & 0x7) as usize
}
