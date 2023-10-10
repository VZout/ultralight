use std::{ffi::CString, ptr::null_mut};

use crate::{
    sys::{
        ulViewLockJSContext, ulViewUnlockJSContext, JSContextRef, JSEvaluateScript,
        JSObjectCallAsFunction, JSObjectMakeArray, JSObjectRef, JSStringCreateWithUTF8CString,
        JSStringRelease, JSValueMakeString, JSValueRef, JSValueToObject,
    },
    View,
};

pub struct JSContext<'a> {
    owner: &'a View,
    inner: JSContextRef,
}

pub trait IntoJSValue {
    fn into_value(self, ctx: &JSContext<'_>) -> JSValueRef;
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

pub trait FucntionCallback {
    fn test(
        _ctx: JSContextRef,
        function: JSObjectRef,
        this_object: JSObjectRef,
        _argument_count: usize,
        _arguments: *const JSValueRef,
        _exception: *mut JSValueRef,
    ) -> JSValueRef;
}

pub trait IntoJSObject {
    fn into_obj(self, ctx: &JSContext<'_>) -> JSObjectRef;
}

impl IntoJSObject for &[&str] {
    fn into_obj(self, ctx: &JSContext<'_>) -> JSObjectRef {
        let array = self
            .iter()
            .map(|name| name.into_value(ctx))
            .collect::<Vec<_>>();

        unsafe { JSObjectMakeArray(ctx.inner, array.len(), array.as_ptr(), null_mut()) }
    }
}

impl<const N: usize> IntoJSObject for [&str; N] {
    fn into_obj(self, ctx: &JSContext<'_>) -> JSObjectRef {
        let array = self
            .iter()
            .map(|name| name.into_value(ctx))
            .collect::<Vec<_>>();

        unsafe { JSObjectMakeArray(ctx.inner, array.len(), array.as_ptr(), null_mut()) }
    }
}

impl<T, const N: usize> IntoJSObject for [T; N]
where
    T: IntoJSObject,
{
    fn into_obj(self, ctx: &JSContext<'_>) -> JSObjectRef {
        let array = self
            .into_iter()
            .map(|v| v.into_obj(ctx))
            .collect::<Vec<_>>();

        unsafe {
            JSObjectMakeArray(
                ctx.inner,
                array.len(),
                array.as_ptr() as *const _,
                null_mut(),
            )
        }
    }
}

impl<T> IntoJSObject for Vec<T>
where
    T: IntoJSObject,
{
    fn into_obj(self, ctx: &JSContext<'_>) -> JSObjectRef {
        let array = self
            .into_iter()
            .map(|v| v.into_obj(ctx))
            .collect::<Vec<_>>();

        unsafe {
            JSObjectMakeArray(
                ctx.inner,
                array.len(),
                array.as_ptr() as *const _,
                null_mut(),
            )
        }
    }
}

impl<const N: usize> IntoJSObject for [JSObjectRef; N] {
    fn into_obj(self, ctx: &JSContext<'_>) -> JSObjectRef {
        unsafe {
            JSObjectMakeArray(
                ctx.into(),
                self.len(),
                self.as_ptr() as *const _,
                null_mut(),
            )
        }
    }
}

impl<'a> JSContext<'a> {
    pub fn new(view: &'a View) -> Self {
        let context = unsafe { ulViewLockJSContext(view.into()) };
        Self {
            owner: view,
            inner: context,
        }
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

    pub fn call_function(&self, func: JSObjectRef, arguments: Vec<JSObjectRef>) -> JSValueRef {
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
        unsafe {
            ulViewUnlockJSContext(self.owner.into());
        }
    }
}

impl From<&JSContext<'_>> for JSContextRef {
    fn from(value: &JSContext) -> Self {
        value.inner
    }
}
