use std::{
    any::Any,
    sync::{Mutex, OnceLock},
};

use crate::sys::{
    ulPlatformSetGPUDriver, C_Bitmap, ULCommandList, ULGPUDriver, ULIndexBuffer, ULRenderBuffer,
    ULVertexBuffer,
};

unsafe extern "C" fn begin_synchronize() {}
unsafe extern "C" fn end_synchronize() {}
unsafe extern "C" fn next_texture_id() -> u32 {
    let mut driver = static_gpu_driver().lock().unwrap();
    let driver = driver.as_mut().expect("Gpu driver enabled but not set?");
    driver.next_texture_id()
}
unsafe extern "C" fn create_texture(id: u32, bitmap: *mut C_Bitmap) {
    let mut driver = static_gpu_driver().lock().unwrap();
    let driver = driver.as_mut().expect("Gpu driver enabled but not set?");
    driver.create_texture(id, bitmap);
}
unsafe extern "C" fn update_texture(id: u32, bitmap: *mut C_Bitmap) {
    let mut driver = static_gpu_driver().lock().unwrap();
    let driver = driver.as_mut().expect("Gpu driver enabled but not set?");
    driver.update_texture(id, bitmap);
}
unsafe extern "C" fn destroy_texture(id: u32) {
    let mut driver = static_gpu_driver().lock().unwrap();
    let driver = driver.as_mut().expect("Gpu driver enabled but not set?");
    driver.destroy_texture(id);
}
unsafe extern "C" fn next_render_buffer_id() -> u32 {
    let mut driver = static_gpu_driver().lock().unwrap();
    let driver = driver.as_mut().expect("Gpu driver enabled but not set?");
    driver.next_render_buffer_id()
}
unsafe extern "C" fn create_render_buffer(id: u32, render_buffer: ULRenderBuffer) {
    let mut driver = static_gpu_driver().lock().unwrap();
    let driver = driver.as_mut().expect("Gpu driver enabled but not set?");
    driver.create_render_buffer(id, render_buffer);
}
unsafe extern "C" fn destroy_render_buffer(id: u32) {
    let mut driver = static_gpu_driver().lock().unwrap();
    let driver = driver.as_mut().expect("Gpu driver enabled but not set?");
    driver.destroy_render_buffer(id);
}
unsafe extern "C" fn next_geometry_id() -> u32 {
    let mut driver = static_gpu_driver().lock().unwrap();
    let driver = driver.as_mut().expect("Gpu driver enabled but not set?");
    driver.next_geometry_id()
}
unsafe extern "C" fn create_geometry(id: u32, vb: ULVertexBuffer, ib: ULIndexBuffer) {
    let mut driver = static_gpu_driver().lock().unwrap();
    let driver = driver.as_mut().expect("Gpu driver enabled but not set?");
    driver.create_geometry(id, vb, ib);
}
unsafe extern "C" fn update_geometry(id: u32, vb: ULVertexBuffer, ib: ULIndexBuffer) {
    let mut driver = static_gpu_driver().lock().unwrap();
    let driver = driver.as_mut().expect("Gpu driver enabled but not set?");
    driver.update_geometry(id, vb, ib);
}
unsafe extern "C" fn destroy_geometry(id: u32) {
    let mut driver = static_gpu_driver().lock().unwrap();
    let driver = driver.as_mut().expect("Gpu driver enabled but not set?");
    driver.destroy_geometry(id);
}
unsafe extern "C" fn update_command_list(cmd_list: ULCommandList) {
    let mut driver = static_gpu_driver().lock().unwrap();
    let driver = driver.as_mut().expect("Gpu driver enabled but not set?");
    driver.update_command_list(cmd_list);
}

pub trait GpuDriver: Send + Sync + Any {
    fn as_any(&self) -> &dyn Any;

    fn next_texture_id(&mut self) -> u32;
    fn create_texture(&mut self, id: u32, bitmap: *mut C_Bitmap);
    fn update_texture(&mut self, id: u32, bitmap: *mut C_Bitmap);
    fn destroy_texture(&mut self, id: u32);

    fn next_render_buffer_id(&mut self) -> u32;
    fn create_render_buffer(&mut self, id: u32, render_buffer: ULRenderBuffer);
    fn destroy_render_buffer(&mut self, id: u32);

    fn next_geometry_id(&mut self) -> u32;
    fn create_geometry(&mut self, id: u32, vb: ULVertexBuffer, ib: ULIndexBuffer);
    fn update_geometry(&mut self, id: u32, vb: ULVertexBuffer, ib: ULIndexBuffer);
    fn destroy_geometry(&mut self, id: u32);

    fn update_command_list(&mut self, cmd_list: ULCommandList);
}

pub fn set_gpu_driver(driver: Box<dyn GpuDriver>) {
    let mut static_driver = static_gpu_driver().lock().unwrap();
    *static_driver = Some(driver);

    unsafe {
        let driver = ULGPUDriver {
            begin_synchronize: Some(begin_synchronize),
            end_synchronize: Some(end_synchronize),
            next_texture_id: Some(next_texture_id),
            create_texture: Some(create_texture),
            update_texture: Some(update_texture),
            destroy_texture: Some(destroy_texture),
            next_render_buffer_id: Some(next_render_buffer_id),
            create_render_buffer: Some(create_render_buffer),
            destroy_render_buffer: Some(destroy_render_buffer),
            next_geometry_id: Some(next_geometry_id),
            create_geometry: Some(create_geometry),
            update_geometry: Some(update_geometry),
            destroy_geometry: Some(destroy_geometry),
            update_command_list: Some(update_command_list),
        };
        ulPlatformSetGPUDriver(driver);
    }
}

pub fn static_gpu_driver() -> &'static Mutex<Option<Box<dyn GpuDriver>>> {
    static ARRAY: OnceLock<Mutex<Option<Box<dyn GpuDriver>>>> = OnceLock::new();
    ARRAY.get_or_init(|| Mutex::new(None))
}
