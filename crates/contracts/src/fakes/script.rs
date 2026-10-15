use vellum_core::ports::{ScriptApi, ScriptError, ScriptLimits, ScriptOutput, ScriptRuntime};

/// Runs nothing: every script finishes at once with no value and no logs.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopScriptRuntime;

impl ScriptRuntime for NoopScriptRuntime {
    fn run(
        &self,
        _src: &str,
        _api: std::rc::Rc<dyn ScriptApi>,
        _limits: ScriptLimits,
    ) -> Result<ScriptOutput, ScriptError> {
        Ok(ScriptOutput::default())
    }
}
