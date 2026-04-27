use crate::{adapters::ai::openai::OpenAiClassifier, config::AppConfig, ports::ai::AiClassifier};

pub fn build_ai_classifier(cfg: &AppConfig) -> Box<dyn AiClassifier> {
    Box::new(OpenAiClassifier::new(&cfg.openai))
}
