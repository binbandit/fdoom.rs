//! Port of `fdoom.core.MyUtils`.

pub fn clamp(val: i32, min: i32, max: i32) -> i32 {
    if val > max {
        max
    } else if val < min {
        min
    } else {
        val
    }
}

/// Java `MyUtils.randInt(min, max)` (Java used `Math.random()`; we thread the game RNG).
pub fn rand_int(random: &mut crate::rng::Rng, min: i32, max: i32) -> i32 {
    (random.next_double() * (max - min + 1) as f64) as i32 + min
}

/// Java `MyUtils.plural(count, word)`.
pub fn plural(amount: i32, noun: &str) -> String {
    let mut s = format!("{amount} {noun}");
    if amount != 1 {
        s.push('s');
    }
    s
}

/// Java `MyUtils.sleep(millis)`.
pub fn sleep(millis: u64) {
    std::thread::sleep(std::time::Duration::from_millis(millis));
}
