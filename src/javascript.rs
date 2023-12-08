use std::{ffi::CString, os::raw::c_void, ptr::null_mut};

use crate::{
    sys::{
        ulViewLockJSContext, ulViewUnlockJSContext, JSContextGetGlobalObject, JSContextRef,
        JSEvaluateScript, JSObjectCallAsFunction, JSObjectGetProperty, JSObjectMake,
        JSObjectMakeArray, JSObjectMakeFunctionWithCallback, JSObjectRef, JSObjectSetProperty,
        JSStringCreateWithUTF8CString, JSStringRelease, JSValueMakeNumber, JSValueMakeString,
        JSValueRef, JSValueToNumber, JSValueToObject,
    },
    View,
};

pub type RustCallback = dyn FnMut(&JSContext<'_>, &[JSValueRef]);

pub trait IntoJSValue {
    fn into_value(self, ctx: &JSContext<'_>) -> JSValueRef;
    fn from_value(ctx: &JSContext<'_>, value: JSValueRef) -> Self
    where
        Self: Sized,
    {
        let _ = ctx;
        let _ = value;
        unimplemented!();
    }
}

pub trait FromJSValue {
    fn from_value(self, ctx: &JSContext<'_>, value: JSValueRef) -> JSValueRef;
}

impl IntoJSValue for &str {
    fn into_value(self, ctx: &JSContext<'_>) -> JSValueRef {
        let string = CString::new(self).unwrap();
        unsafe {
            let utf8 = JSStringCreateWithUTF8CString(string.as_ptr());
            let value = JSValueMakeString(ctx.inner, utf8);
            JSStringRelease(utf8);
            value
        }
    }
}

impl IntoJSValue for f64 {
    fn into_value(self, ctx: &JSContext<'_>) -> JSValueRef {
        unsafe { JSValueMakeNumber(ctx.inner, self) }
    }

    fn from_value(ctx: &JSContext<'_>, value: JSValueRef) -> Self {
        unsafe { JSValueToNumber(ctx.inner, value, null_mut()) }
    }
}

impl IntoJSValue for f32 {
    fn into_value(self, ctx: &JSContext<'_>) -> JSValueRef {
        unsafe { JSValueMakeNumber(ctx.inner, self as f64) }
    }

    fn from_value(ctx: &JSContext<'_>, value: JSValueRef) -> Self {
        unsafe { JSValueToNumber(ctx.inner, value, null_mut()) as Self }
    }
}

impl IntoJSValue for u32 {
    fn into_value(self, ctx: &JSContext<'_>) -> JSValueRef {
        unsafe { JSValueMakeNumber(ctx.inner, self as f64) }
    }

    fn from_value(ctx: &JSContext<'_>, value: JSValueRef) -> Self {
        unsafe { JSValueToNumber(ctx.inner, value, null_mut()) as Self }
    }
}

impl IntoJSValue for String {
    fn into_value(self, ctx: &JSContext<'_>) -> JSValueRef {
        let string = CString::new(self).unwrap();
        unsafe {
            let utf8 = JSStringCreateWithUTF8CString(string.as_ptr());
            let value = JSValueMakeString(ctx.inner, utf8);
            JSStringRelease(utf8);
            value
        }
    }
}

impl<T: IntoJSValue> IntoJSObject for T {
    fn into_obj<'a>(self, ctx: &'a JSContext<'a>) -> JSObject<'a> {
        JSObject::from_value(ctx, self.into_value(ctx))
    }
}

pub trait IntoJSObject {
    fn into_obj<'a>(self, ctx: &'a JSContext<'a>) -> JSObject<'a>;
}

impl IntoJSObject for &[&str] {
    fn into_obj<'a>(self, ctx: &'a JSContext<'a>) -> JSObject<'a> {
        let array = self
            .iter()
            .map(|name| name.into_value(ctx))
            .collect::<Vec<_>>();

        let inner =
            unsafe { JSObjectMakeArray(ctx.inner, array.len(), array.as_ptr(), null_mut()) };

        JSObject::from_object(ctx, inner)
    }
}

impl<T, const N: usize> IntoJSObject for [T; N]
where
    T: IntoJSObject,
{
    fn into_obj<'a>(self, ctx: &'a JSContext<'a>) -> JSObject<'a> {
        let array = self
            .into_iter()
            .map(|v| v.into_obj(ctx).inner)
            .collect::<Vec<_>>();

        let inner = unsafe {
            JSObjectMakeArray(
                ctx.inner,
                array.len(),
                array.as_ptr() as *const _,
                null_mut(),
            )
        };
        JSObject::from_object(ctx, inner)
    }
}

impl<T> IntoJSObject for Vec<T>
where
    T: IntoJSObject,
{
    fn into_obj<'a>(self, ctx: &'a JSContext<'a>) -> JSObject<'a> {
        let array = self
            .into_iter()
            .map(|v| v.into_obj(ctx).inner)
            .collect::<Vec<_>>();

        let inner = unsafe {
            JSObjectMakeArray(
                ctx.inner,
                array.len(),
                array.as_ptr() as *const _,
                null_mut(),
            )
        };
        JSObject::from_object(ctx, inner)
    }
}

impl<const N: usize> IntoJSObject for [JSObjectRef; N] {
    fn into_obj<'a>(self, ctx: &'a JSContext<'a>) -> JSObject<'a> {
        let inner = unsafe {
            JSObjectMakeArray(
                ctx.into(),
                self.len(),
                self.as_ptr() as *const _,
                null_mut(),
            )
        };
        JSObject::from_object(ctx, inner)
    }
}

pub struct JSContext<'a> {
    owner: Option<&'a View>,
    inner: JSContextRef,
}

impl<'a> JSContext<'a> {
    pub fn new(view: &'a View) -> Self {
        let context = unsafe { ulViewLockJSContext(view.into()) };
        Self {
            owner: Some(view),
            inner: context,
        }
    }

    pub fn get_global_object(&self) -> JSObject<'_> {
        let global_object = unsafe { JSContextGetGlobalObject(self.inner) };
        JSObject::from_object(self, global_object)
    }

    // TODO: Return option
    // TODO: Wrap in wrapper so you can call functions directly?
    pub fn get_function(&self, name: &str) -> JSObjectRef {
        let name = CString::new(name).unwrap();

        unsafe {
            let name = JSStringCreateWithUTF8CString(name.as_ptr());
            let func = JSEvaluateScript(self.inner, name, null_mut(), null_mut(), 0, null_mut());
            JSStringRelease(name);
            JSValueToObject(self.inner, func, null_mut())
        }
    }

    pub fn call_function(&self, func: JSObjectRef, arguments: Vec<JSObject<'_>>) -> JSValueRef {
        let arguments: Vec<_> = arguments.iter().map(|o| o.inner).collect();

        unsafe {
            JSObjectCallAsFunction(
                self.inner,
                func,
                null_mut(),
                arguments.len(),
                arguments.as_ptr() as _,
                null_mut(),
            )
        }
    }
}

impl Drop for JSContext<'_> {
    fn drop(&mut self) {
        if let Some(owner) = self.owner.take() {
            unsafe {
                ulViewUnlockJSContext(owner.into());
            }
        }
    }
}

impl From<JSContextRef> for JSContext<'_> {
    fn from(value: JSContextRef) -> Self {
        Self {
            owner: None,
            inner: value,
        }
    }
}

impl From<&JSContext<'_>> for JSContextRef {
    fn from(value: &JSContext<'_>) -> Self {
        value.inner
    }
}

/* ========================= */
/*         JSObject          */
/* ========================= */

pub struct JSObject<'a> {
    ctx: &'a JSContext<'a>,
    inner: JSObjectRef,
}

impl<'a> JSObject<'a> {
    pub fn new(ctx: &'a JSContext<'a>) -> Self {
        let inner = unsafe { JSObjectMake(ctx.into(), null_mut(), null_mut()) };

        Self { ctx, inner }
    }

    // TODO: Make private as its build using a sys type.
    pub fn from_object(ctx: &'a JSContext<'a>, inner: JSObjectRef) -> Self {
        Self { ctx, inner }
    }

    // TODO: Make private as its build using a sys type.
    pub fn from_value(ctx: &'a JSContext<'a>, inner: JSValueRef) -> Self {
        Self {
            ctx,
            inner: inner as _,
        }
    }

    pub fn set_property(&mut self, name: &str, property_object: impl IntoJSObject) {
        unsafe {
            let name = CString::new(name).unwrap();
            let name = JSStringCreateWithUTF8CString(name.as_ptr());
            JSObjectSetProperty(
                self.ctx.into(),
                self.inner,
                name,
                property_object.into_obj(self.ctx).inner,
                0,
                null_mut(),
            );
            JSStringRelease(name);
        }
    }

    pub fn set_rust_callback(&mut self, function_name: &str, callback: Box<Box<RustCallback>>) {
        unsafe {
            // callback field
            let func_obj = {
                let prop_name = CString::new(function_name).unwrap();
                let prop_name = JSStringCreateWithUTF8CString(prop_name.as_ptr());
                let func = JSObjectMakeFunctionWithCallback(
                    self.ctx.into(),
                    prop_name,
                    Some(callback_wrapper),
                );
                JSObjectSetProperty(self.ctx.into(), self.inner, prop_name, func, 0, null_mut());
                JSStringRelease(prop_name);

                func
            };

            // private callback field
            {
                let prop_name = CString::new("internalpointer").unwrap();
                let prop_name = JSStringCreateWithUTF8CString(prop_name.as_ptr());

                // TODO: I think this is a memory leak.
                // TODO: Actually im pretty sure. See https://stackoverflow.com/questions/32270030/how-do-i-convert-a-rust-closure-to-a-c-style-callback to how to unset
                let func_pointer = Box::into_raw(callback) as *mut _;

                let func = JSValueMakeNumber(self.ctx.into(), f64::from_bits(func_pointer as u64));
                JSObjectSetProperty(self.ctx.into(), func_obj, prop_name, func, 0, null_mut());
                JSStringRelease(prop_name);
            }
        }
    }
}

impl From<&JSObject<'_>> for JSObjectRef {
    fn from(value: &JSObject<'_>) -> Self {
        value.inner
    }
}

impl Drop for JSObject<'_> {
    fn drop(&mut self) {}
}

/// Wraps rust callbacks for `JSObject`
extern "C" fn callback_wrapper(
    ctx: JSContextRef,
    function: JSObjectRef,
    _this_object: JSObjectRef,
    argument_count: usize,
    arguments: *const JSValueRef,
    _exception: *mut JSValueRef,
) -> JSValueRef {
    unsafe {
        let prop_name = CString::new("internalpointer").unwrap();
        let prop_name = JSStringCreateWithUTF8CString(prop_name.as_ptr());
        let internalpointer = JSObjectGetProperty(ctx, function, prop_name, null_mut());
        JSStringRelease(prop_name);

        let ptr =
            JSValueToNumber(ctx, internalpointer, null_mut()).to_bits() as usize as *mut c_void;
        let closure: &mut Box<RustCallback> = std::mem::transmute(ptr);

        // Closure arguments
        let ctx = JSContext::from(ctx);
        let arguments = std::slice::from_raw_parts(arguments, argument_count);

        closure(&ctx, arguments);
    }

    std::ptr::null()
}
