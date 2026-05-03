use anyhow::Result;
use kanata_parser::custom_action::CustomAction;

pub fn set_current_input_source_by_id(id: &str) -> Result<()> {
    backend::set_current_input_source_by_id(id)
}

fn current_input_source_is(id: &str) -> Result<bool> {
    backend::current_input_source_is(id)
}

pub fn evaluate_custom_condition(action: &&'static CustomAction) -> bool {
    match action {
        CustomAction::InputSourceIs(id) => match current_input_source_is(id) {
            Ok(is_current) => is_current,
            Err(e) => {
                log::error!("failed to check macOS input source: {e}");
                false
            }
        },
        _ => false,
    }
}

#[cfg(target_os = "macos")]
mod backend {
    use anyhow::Result;

    pub fn set_current_input_source_by_id(id: &str) -> Result<()> {
        crate::macos_input_source::set_current_input_source_by_id_via_helper(id)
    }

    pub fn current_input_source_is(id: &str) -> Result<bool> {
        crate::macos_input_source::current_input_source_is_via_helper(id)
    }
}

#[cfg(not(target_os = "macos"))]
mod backend {
    use anyhow::{Result, bail};

    pub fn set_current_input_source_by_id(_id: &str) -> Result<()> {
        bail!("set-input-source is only supported on macOS")
    }

    pub fn current_input_source_is(_id: &str) -> Result<bool> {
        bail!("input-source-is is only supported on macOS")
    }
}
