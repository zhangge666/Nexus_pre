//! 本文件定义可替换的文本嵌入接口和用于最小闭环的确定性哈希嵌入器。

/// 抽象本地 ONNX 或远程嵌入模型需要提供的能力。
pub trait Embedder: Send + Sync {
    /// 返回当前向量空间的维度。
    fn dimension(&self) -> usize;

    /// 将一段文本转换为单位向量。
    fn embed(&self, text: &str) -> Vec<f32>;
}

/// 使用字符三元组生成确定性向量，供 ONNX 模型接入前验证完整数据链路。
#[derive(Debug, Clone)]
pub struct HashEmbedder {
    dimension: usize,
}

impl HashEmbedder {
    /// 创建指定维度的哈希嵌入器，维度至少为 8。
    #[must_use]
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension: dimension.max(8),
        }
    }
}

impl Default for HashEmbedder {
    /// 创建适合测试与最小本地闭环的 64 维嵌入器。
    fn default() -> Self {
        Self::new(64)
    }
}

impl Embedder for HashEmbedder {
    /// 返回哈希向量空间维度。
    fn dimension(&self) -> usize {
        self.dimension
    }

    /// 将规范化文本的字符窗口散列到固定维度并执行 L2 归一化。
    fn embed(&self, text: &str) -> Vec<f32> {
        let normalized = text.to_lowercase();
        let chars = normalized.chars().collect::<Vec<_>>();
        let mut vector = vec![0.0_f32; self.dimension];

        // 同时记录单字符和三字符窗口，让中文短查询与英文词组都能保留局部语义信号。
        for window_size in [1, 3] {
            for window in chars.windows(window_size) {
                let mut hash = 2_166_136_261_u64;
                for character in window {
                    hash ^= u64::from(*character as u32);
                    hash = hash.wrapping_mul(16_777_619);
                }
                vector[hash as usize % self.dimension] += 1.0;
            }
        }

        let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        if norm > 0.0 {
            vector.iter_mut().for_each(|value| *value /= norm);
        }
        vector
    }
}

/// 计算两个单位向量的余弦相似度，不同维度只比较公共部分。
#[must_use]
pub fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    left.iter().zip(right).map(|(a, b)| a * b).sum()
}
