//! Boa JavaScript runtime integration for TUI automation scripting.
//!
//! This module provides a JavaScript execution environment with a `tui` global object
//! that exposes TUI automation methods like `tui.text()`, `tui.sendText()`, etc.

use boa_engine::{
    js_string, native_function::NativeFunction, object::FunctionObjectBuilder, property::Attribute,
    Context, JsArgs, JsResult, JsString, JsValue, Source,
};
use tui_driver::{Key, TuiDriver};

/// Execute a JavaScript script with access to TUI automation functions.
///
/// The script has access to a global `tui` object with the following methods:
/// - `tui.text()` - Returns the current screen text
/// - `tui.sendText(text)` - Sends text to the terminal
/// - `tui.pressKey(key)` - Presses a key (e.g., "Enter", "Ctrl+c")
/// - `tui.clickAt(x, y)` - Clicks at the specified coordinates
/// - `tui.snapshot()` - Returns a YAML accessibility snapshot
///
/// # Arguments
///
/// * `driver` - Reference to the TuiDriver instance
/// * `code` - JavaScript code to execute
///
/// # Returns
///
/// Returns the string representation of the last evaluated expression,
/// or an error message if execution fails.
pub fn execute_script(driver: &TuiDriver, code: &str) -> Result<String, String> {
    // Create a new JavaScript context
    let mut context = Context::default();

    // We need to wrap the driver in an Arc for sharing across closures.
    // Since TuiDriver uses interior mutability, this is safe.
    // We create a "fake" Arc by using a raw pointer - this is safe because
    // the driver reference outlives the script execution.
    //
    // SAFETY: The driver reference is valid for the duration of execute_script,
    // and all JavaScript execution happens synchronously within this function.
    let driver_ptr = driver as *const TuiDriver;

    // Create the tui object
    let tui_object = create_tui_object(&mut context, driver_ptr)
        .map_err(|e| format!("Failed to create tui object: {}", e))?;

    // Register the tui object as a global property
    context
        .register_global_property(
            js_string!("tui"),
            tui_object,
            Attribute::READONLY | Attribute::NON_ENUMERABLE | Attribute::PERMANENT,
        )
        .map_err(|e| format!("Failed to register tui global: {}", e))?;

    // Execute the JavaScript code
    let result = context
        .eval(Source::from_bytes(code))
        .map_err(|e| format!("JavaScript error: {}", e))?;

    // Convert result to string
    let result_str = result
        .to_string(&mut context)
        .map_err(|e| format!("Failed to convert result: {}", e))?;

    Ok(result_str.to_std_string_escaped())
}

/// Create the `tui` JavaScript object with all automation methods.
fn create_tui_object(context: &mut Context, driver_ptr: *const TuiDriver) -> JsResult<JsValue> {
    // Create an empty object for tui
    let tui_obj = boa_engine::JsObject::with_null_proto();

    // Add tui.text() method
    let text_fn = create_text_method(context, driver_ptr);
    tui_obj.set(js_string!("text"), text_fn, false, context)?;

    // Add tui.sendText(text) method
    let send_text_fn = create_send_text_method(context, driver_ptr);
    tui_obj.set(js_string!("sendText"), send_text_fn, false, context)?;

    // Add tui.pressKey(key) method
    let press_key_fn = create_press_key_method(context, driver_ptr);
    tui_obj.set(js_string!("pressKey"), press_key_fn, false, context)?;

    // Add tui.clickAt(x, y) method
    let click_at_fn = create_click_at_method(context, driver_ptr);
    tui_obj.set(js_string!("clickAt"), click_at_fn, false, context)?;

    // Add tui.snapshot() method
    let snapshot_fn = create_snapshot_method(context, driver_ptr);
    tui_obj.set(js_string!("snapshot"), snapshot_fn, false, context)?;

    Ok(tui_obj.into())
}

/// Create the tui.text() method that returns screen text.
fn create_text_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    // We use a raw pointer here because the driver lifetime is guaranteed
    // to outlive the script execution (synchronous execution in execute_script)
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, _args, _ctx| {
            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };
            let text = driver.text();
            Ok(JsValue::from(JsString::from(text.as_str())))
        }),
    )
    .name(js_string!("text"))
    .length(0)
    .build()
    .into()
}

/// Create the tui.sendText(text) method.
fn create_send_text_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            // Get the text argument
            let text = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_escaped();

            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };

            match driver.send_text(&text) {
                Ok(()) => Ok(JsValue::undefined()),
                Err(e) => Err(boa_engine::JsError::from_opaque(JsValue::from(
                    JsString::from(format!("sendText error: {}", e).as_str()),
                ))),
            }
        }),
    )
    .name(js_string!("sendText"))
    .length(1)
    .build()
    .into()
}

/// Create the tui.pressKey(key) method.
fn create_press_key_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            // Get the key argument
            let key_str = args
                .get_or_undefined(0)
                .to_string(ctx)?
                .to_std_string_escaped();

            // Parse the key
            let key = Key::parse(&key_str).map_err(|e| {
                boa_engine::JsError::from_opaque(JsValue::from(JsString::from(
                    format!("Invalid key '{}': {}", key_str, e).as_str(),
                )))
            })?;

            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };

            match driver.press_key(&key) {
                Ok(()) => Ok(JsValue::undefined()),
                Err(e) => Err(boa_engine::JsError::from_opaque(JsValue::from(
                    JsString::from(format!("pressKey error: {}", e).as_str()),
                ))),
            }
        }),
    )
    .name(js_string!("pressKey"))
    .length(1)
    .build()
    .into()
}

/// Create the tui.clickAt(x, y) method.
fn create_click_at_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, args, ctx| {
            // Get x and y arguments
            let x = args.get_or_undefined(0).to_u32(ctx)? as u16;
            let y = args.get_or_undefined(1).to_u32(ctx)? as u16;

            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };

            match driver.click_at(x, y) {
                Ok(()) => Ok(JsValue::undefined()),
                Err(e) => Err(boa_engine::JsError::from_opaque(JsValue::from(
                    JsString::from(format!("clickAt error: {}", e).as_str()),
                ))),
            }
        }),
    )
    .name(js_string!("clickAt"))
    .length(2)
    .build()
    .into()
}

/// Create the tui.snapshot() method that returns YAML snapshot.
fn create_snapshot_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, _args, _ctx| {
            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };
            let snapshot = driver.snapshot();
            let yaml = snapshot.yaml.unwrap_or_default();
            Ok(JsValue::from(JsString::from(yaml.as_str())))
        }),
    )
    .name(js_string!("snapshot"))
    .length(0)
    .build()
    .into()
}

#[cfg(test)]
mod tests {
    // Note: Full integration tests require a running TUI session.
    // Unit tests for basic JavaScript execution would require mocking TuiDriver.
}
