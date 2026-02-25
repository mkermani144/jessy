use jessy_core::JobStage;

pub const STEP_STAGE: JobStage = JobStage::Load;

pub const fn step_name() -> &'static str {
    STEP_STAGE.as_str()
}
