//! Evidence-based execution classification shared by live events and recovery.
//! Unknown Python errors remain ordinary failures; never infer compatibility
//! from workflow names, prompt text, or a ComfyUI version alone.
pub(crate) const NODE_INCOMPATIBLE: &str = "COMFY_NODE_INCOMPATIBLE";

pub(crate) fn execution_error_code(message: &str) -> &'static str {
    if message.contains("FinalLayer.forward()")
        && message.contains("missing 3 required positional arguments")
        && ["sigma", "sample_sigmas", "shifts"]
            .iter()
            .all(|argument| message.contains(argument))
    {
        NODE_INCOMPATIBLE
    } else {
        "EXECUTION_ERROR"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_the_observed_signature_mismatch_not_arbitrary_errors() {
        assert_eq!(execution_error_code("node 3: FinalLayer.forward() missing 3 required positional arguments: 'sigma', 'sample_sigmas', and 'shifts'"), NODE_INCOMPATIBLE);
        for message in [
            "CUDA out of memory",
            "image not found",
            "unknown execution error",
            "FinalLayer.forward() missing 1 required positional argument: 'x'",
            "missing 3 required positional arguments: 'sigma', 'sample_sigmas', and 'shifts'",
        ] {
            assert_eq!(execution_error_code(message), "EXECUTION_ERROR");
        }
    }
}
