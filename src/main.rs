use salvation_metal::ffi::device_name;

fn main() {
    let a = device_name().unwrap();
    
    println!("{}", a);
}
