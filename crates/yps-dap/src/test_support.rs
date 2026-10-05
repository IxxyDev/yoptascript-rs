pub fn fixture(name: &str) -> String {
    format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
}

pub struct Lcg(pub u64);

impl Lcg {
    pub const fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }

    pub const fn below(&mut self, bound: usize) -> usize {
        (self.next() % bound as u64) as usize
    }

    pub fn pick<T: Clone>(&mut self, items: &[T]) -> T {
        items[self.below(items.len())].clone()
    }

    pub fn bytes(&mut self, max_len: usize) -> Vec<u8> {
        let len = self.below(max_len + 1);
        (0..len).map(|_| (self.next() & 0xff) as u8).collect()
    }
}
