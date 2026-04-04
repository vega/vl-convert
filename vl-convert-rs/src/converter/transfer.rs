use deno_core::anyhow::anyhow;
use deno_core::error::AnyError;
use deno_core::op2;
use deno_core::OpState;
use deno_error::JsErrorBox;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::value_or_string::ValueOrString;

#[derive(Default)]
pub(crate) struct WorkerTransferState {
    json_args: HashMap<i32, String>,
    msgpack_results: HashMap<i32, Vec<u8>>,
    next_id: i32,
}

pub(crate) type WorkerTransferStateHandle = Rc<RefCell<WorkerTransferState>>;

pub(crate) fn next_id(transfer_state: &WorkerTransferStateHandle) -> Result<i32, AnyError> {
    let mut guard = transfer_state
        .try_borrow_mut()
        .map_err(|err| anyhow!("Failed to borrow worker transfer state: {err}"))?;
    let id = guard.next_id;
    guard.next_id = (guard.next_id + 1) % i32::MAX;
    Ok(id)
}

pub(crate) fn set_json_arg(
    transfer_state: &WorkerTransferStateHandle,
    arg: serde_json::Value,
) -> Result<i32, AnyError> {
    set_json_str_arg(transfer_state, serde_json::to_string(&arg)?)
}

pub(crate) fn set_json_str_arg(
    transfer_state: &WorkerTransferStateHandle,
    json_str: String,
) -> Result<i32, AnyError> {
    let id = next_id(transfer_state)?;
    let mut guard = transfer_state
        .try_borrow_mut()
        .map_err(|err| anyhow!("Failed to borrow worker transfer state: {err}"))?;
    guard.json_args.insert(id, json_str);
    Ok(id)
}

pub(crate) fn set_spec_arg(
    transfer_state: &WorkerTransferStateHandle,
    spec: ValueOrString,
) -> Result<i32, AnyError> {
    match spec {
        ValueOrString::JsonString(s) => set_json_str_arg(transfer_state, s),
        ValueOrString::Value(v) => set_json_arg(transfer_state, v),
    }
}

pub(crate) fn alloc_msgpack_result_id(
    transfer_state: &WorkerTransferStateHandle,
) -> Result<i32, AnyError> {
    next_id(transfer_state)
}

pub(crate) fn take_msgpack_result(
    transfer_state: &WorkerTransferStateHandle,
    result_id: i32,
) -> Result<Vec<u8>, AnyError> {
    let mut guard = transfer_state
        .try_borrow_mut()
        .map_err(|err| anyhow!("Failed to borrow worker transfer state: {err}"))?;
    guard
        .msgpack_results
        .remove(&result_id)
        .ok_or_else(|| anyhow!("Result id not found"))
}

pub(crate) fn clear_json_arg(transfer_state: &WorkerTransferStateHandle, arg_id: i32) {
    if let Ok(mut guard) = transfer_state.try_borrow_mut() {
        guard.json_args.remove(&arg_id);
    }
}

pub(crate) fn clear_msgpack_result(transfer_state: &WorkerTransferStateHandle, result_id: i32) {
    if let Ok(mut guard) = transfer_state.try_borrow_mut() {
        guard.msgpack_results.remove(&result_id);
    }
}

pub(crate) struct JsonArgGuard {
    transfer_state: WorkerTransferStateHandle,
    arg_id: Option<i32>,
}

impl JsonArgGuard {
    pub(crate) fn from_value(
        transfer_state: &WorkerTransferStateHandle,
        value: serde_json::Value,
    ) -> Result<Self, AnyError> {
        Ok(Self {
            transfer_state: transfer_state.clone(),
            arg_id: Some(set_json_arg(transfer_state, value)?),
        })
    }

    pub(crate) fn from_spec(
        transfer_state: &WorkerTransferStateHandle,
        spec: ValueOrString,
    ) -> Result<Self, AnyError> {
        Ok(Self {
            transfer_state: transfer_state.clone(),
            arg_id: Some(set_spec_arg(transfer_state, spec)?),
        })
    }

    pub(crate) fn id(&self) -> i32 {
        self.arg_id.expect("JsonArgGuard id missing")
    }
}

impl Drop for JsonArgGuard {
    fn drop(&mut self) {
        if let Some(arg_id) = self.arg_id.take() {
            clear_json_arg(&self.transfer_state, arg_id);
        }
    }
}

pub(crate) struct MsgpackResultGuard {
    transfer_state: WorkerTransferStateHandle,
    result_id: Option<i32>,
}

impl MsgpackResultGuard {
    pub(crate) fn new(transfer_state: &WorkerTransferStateHandle) -> Result<Self, AnyError> {
        Ok(Self {
            transfer_state: transfer_state.clone(),
            result_id: Some(alloc_msgpack_result_id(transfer_state)?),
        })
    }

    pub(crate) fn id(&self) -> i32 {
        self.result_id.expect("MsgpackResultGuard id missing")
    }

    pub(crate) fn take_result(mut self) -> Result<Vec<u8>, AnyError> {
        let result_id = self
            .result_id
            .take()
            .expect("MsgpackResultGuard id missing");
        take_msgpack_result(&self.transfer_state, result_id)
    }
}

impl Drop for MsgpackResultGuard {
    fn drop(&mut self) {
        if let Some(result_id) = self.result_id.take() {
            clear_msgpack_result(&self.transfer_state, result_id);
        }
    }
}

#[op2]
#[string]
pub(crate) fn op_get_json_arg(state: &mut OpState, arg_id: i32) -> Result<String, JsErrorBox> {
    let transfer_state = state
        .try_borrow::<WorkerTransferStateHandle>()
        .cloned()
        .ok_or_else(|| JsErrorBox::generic("Worker transfer state not found"))?;
    let mut guard = transfer_state.try_borrow_mut().map_err(|err| {
        JsErrorBox::generic(format!("Failed to borrow worker transfer state: {err}"))
    })?;
    if let Some(arg) = guard.json_args.remove(&arg_id) {
        Ok(arg)
    } else {
        Err(JsErrorBox::generic("Arg id not found"))
    }
}

#[op2(fast)]
pub(crate) fn op_set_msgpack_result(
    state: &mut OpState,
    result_id: i32,
    #[buffer] data: &[u8],
) -> Result<(), JsErrorBox> {
    let transfer_state = state
        .try_borrow::<WorkerTransferStateHandle>()
        .cloned()
        .ok_or_else(|| JsErrorBox::generic("Worker transfer state not found"))?;
    let mut guard = transfer_state.try_borrow_mut().map_err(|err| {
        JsErrorBox::generic(format!("Failed to borrow worker transfer state: {err}"))
    })?;
    guard.msgpack_results.insert(result_id, data.to_vec());
    Ok(())
}
