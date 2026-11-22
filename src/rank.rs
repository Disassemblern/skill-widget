pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
    let norm = |v: &[f32]| v.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (norm(a) * norm(b)).max(1e-12)
}

/// Indexes of `docs`, most similar to `query` first.
pub fn rank(query: &[f32], docs: &[Vec<f32>]) -> Vec<(usize, f32)> {
    let mut scored: Vec<(usize, f32)> = docs.iter().map(|d| cosine(query, d)).enumerate().collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closest_first() {
        let docs = vec![vec![0.0, 1.0], vec![1.0, 0.1], vec![-1.0, 0.0]];
        let order: Vec<usize> = rank(&[1.0, 0.0], &docs).into_iter().map(|(i, _)| i).collect();
        assert_eq!(order, vec![1, 0, 2]);
    }

    #[test]
    fn cosine_ignores_length() {
        assert!((cosine(&[2.0, 0.0], &[5.0, 0.0]) - 1.0).abs() < 1e-6);
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 0.0]), 0.0);
    }
}
