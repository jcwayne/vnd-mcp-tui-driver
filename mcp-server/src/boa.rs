//! Boa JavaScript runtime integration for TUI automation scripting.
//!
//! This module provides a JavaScript execution environment with a `tui` global object
//! that exposes TUI automation methods like `tui.text()`, `tui.sendText()`, etc.

use boa_engine::{
    js_string, native_function::NativeFunction,
    object::{builtins::JsArray, FunctionObjectBuilder, JsObject},
    property::Attribute,
    Context, JsArgs, JsResult, JsString, JsValue, Source,
};
use tui_driver::{snapshot::Snapshot, Key, Row, Span, TuiDriver};

/// Execute a JavaScript script with access to TUI automation functions.
///
/// The script has access to a global `tui` object with the following methods:
/// - `tui.text()` - Returns the current screen text
/// - `tui.sendText(text)` - Sends text to the terminal
/// - `tui.pressKey(key)` - Presses a key (e.g., "Enter", "Ctrl+c")
/// - `tui.clickAt(x, y)` - Clicks at the specified coordinates
/// - `tui.snapshot()` - Returns an accessibility snapshot as a JavaScript object
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

/// Convert a Span to a JavaScript object.
///
/// Only includes optional fields if they have truthy values.
fn span_to_js_object(span: &Span, context: &mut Context) -> JsResult<JsObject> {
    let obj = JsObject::with_null_proto();

    // Required fields
    obj.set(
        js_string!("ref"),
        JsValue::from(JsString::from(span.ref_id.as_str())),
        false,
        context,
    )?;
    obj.set(
        js_string!("text"),
        JsValue::from(JsString::from(span.text.as_str())),
        false,
        context,
    )?;
    obj.set(
        js_string!("x"),
        JsValue::from(span.x as i32),
        false,
        context,
    )?;
    obj.set(
        js_string!("y"),
        JsValue::from(span.y as i32),
        false,
        context,
    )?;
    obj.set(
        js_string!("width"),
        JsValue::from(span.width as i32),
        false,
        context,
    )?;

    // Optional boolean fields - only include if true
    if span.bold == Some(true) {
        obj.set(js_string!("bold"), JsValue::from(true), false, context)?;
    }
    if span.italic == Some(true) {
        obj.set(js_string!("italic"), JsValue::from(true), false, context)?;
    }
    if span.underline == Some(true) {
        obj.set(
            js_string!("underline"),
            JsValue::from(true),
            false,
            context,
        )?;
    }
    if span.inverse == Some(true) {
        obj.set(js_string!("inverse"), JsValue::from(true), false, context)?;
    }
    if span.strikethrough == Some(true) {
        obj.set(
            js_string!("strikethrough"),
            JsValue::from(true),
            false,
            context,
        )?;
    }

    // Optional string fields - only include if Some
    if let Some(ref fg) = span.fg {
        obj.set(
            js_string!("fg"),
            JsValue::from(JsString::from(fg.as_str())),
            false,
            context,
        )?;
    }
    if let Some(ref bg) = span.bg {
        obj.set(
            js_string!("bg"),
            JsValue::from(JsString::from(bg.as_str())),
            false,
            context,
        )?;
    }
    if let Some(ref underline_style) = span.underline_style {
        obj.set(
            js_string!("underline_style"),
            JsValue::from(JsString::from(underline_style.as_str())),
            false,
            context,
        )?;
    }
    if let Some(ref blink) = span.blink {
        obj.set(
            js_string!("blink"),
            JsValue::from(JsString::from(blink.as_str())),
            false,
            context,
        )?;
    }
    if let Some(ref link) = span.link {
        obj.set(
            js_string!("link"),
            JsValue::from(JsString::from(link.as_str())),
            false,
            context,
        )?;
    }
    if let Some(ref image) = span.image {
        obj.set(
            js_string!("image"),
            JsValue::from(JsString::from(image.as_str())),
            false,
            context,
        )?;
    }
    if let Some(ref image_size) = span.image_size {
        obj.set(
            js_string!("image_size"),
            JsValue::from(JsString::from(image_size.as_str())),
            false,
            context,
        )?;
    }

    Ok(obj)
}

/// Convert a Row to a JavaScript object.
fn row_to_js_object(row: &Row, context: &mut Context) -> JsResult<JsObject> {
    let obj = JsObject::with_null_proto();

    // Row number
    obj.set(
        js_string!("row_number"),
        JsValue::from(row.row as i32),
        false,
        context,
    )?;

    // Convert spans to JS array
    let spans_array = JsArray::new(context);
    for span in &row.spans {
        let span_obj = span_to_js_object(span, context)?;
        spans_array.push(JsValue::from(span_obj), context)?;
    }
    obj.set(js_string!("spans"), JsValue::from(spans_array), false, context)?;

    Ok(obj)
}

/// Convert a Snapshot to a JavaScript object.
fn snapshot_to_js_object(snapshot: &Snapshot, context: &mut Context) -> JsResult<JsObject> {
    let obj = JsObject::with_null_proto();

    // Convert rows to JS array
    let rows_array = JsArray::new(context);
    for row in &snapshot.rows {
        let row_obj = row_to_js_object(row, context)?;
        rows_array.push(JsValue::from(row_obj), context)?;
    }
    obj.set(js_string!("rows"), JsValue::from(rows_array), false, context)?;

    // Convert flat spans list to JS array
    let spans_array = JsArray::new(context);
    for span in &snapshot.spans {
        let span_obj = span_to_js_object(span, context)?;
        spans_array.push(JsValue::from(span_obj), context)?;
    }
    obj.set(js_string!("spans"), JsValue::from(spans_array), false, context)?;

    // Add span count
    obj.set(
        js_string!("span_count"),
        JsValue::from(snapshot.spans.len() as i32),
        false,
        context,
    )?;

    Ok(obj)
}

/// Create the tui.snapshot() method that returns a structured JavaScript object.
fn create_snapshot_method(context: &mut Context, driver_ptr: *const TuiDriver) -> JsValue {
    let ptr = driver_ptr as usize;

    FunctionObjectBuilder::new(
        context.realm(),
        NativeFunction::from_copy_closure(move |_this, _args, ctx| {
            // SAFETY: ptr was created from a valid reference that outlives this closure
            let driver = unsafe { &*(ptr as *const TuiDriver) };
            let snapshot = driver.snapshot();
            let js_obj = snapshot_to_js_object(&snapshot, ctx)?;
            Ok(JsValue::from(js_obj))
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
