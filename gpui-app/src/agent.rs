//! OpenCode agent panel (ACP). Filled in by the ACP step.

use crate::workbench::Workbench;
use gpui_kit::*;

pub struct AgentState {}

impl AgentState {
    pub fn new(_: &mut Window, _: &mut Context<Workbench>) -> Self {
        Self {}
    }
    pub fn on_output(&mut self, _id: u32, _text: &str, _err: bool) -> Option<String> {
        None
    }
    pub fn on_exit(&mut self, _id: u32) {}
}

impl Workbench {
    pub(crate) fn render_agent(&self, _: &mut Window, _: &mut Context<Self>) -> AnyElement {
        div().p_4().child("OpenCode is not ported yet.").into_any_element()
    }
    pub(crate) fn agent_message(&mut self, _msg: String, _: &mut Context<Self>) {}
}
