use crate::AiError;

pub struct LocalLlm;

impl LocalLlm {
    pub fn placeholder() -> Result<(), AiError> {
        Ok(())
    }
}
