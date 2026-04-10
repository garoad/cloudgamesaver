fn main() {
    // 프로젝트 루트에 있는 .env 파일을 읽어서 환경 변수로 설정합니다.
    println!("cargo:rerun-if-changed=../.env");
    let _ = dotenvy::from_path("../.env");
    
    // 환경 변수가 존재하면 Rust 컴파일러에게 전달하여 
    // option_env! 매크로가 이를 인식할 수 있게 합니다.
    if let Ok(key) = std::env::var("APP_KEY") {
        println!("cargo:rustc-env=APP_KEY={}", key);
    } else {
        println!("cargo:warning=APP_KEY is not set in .env file");
    }
    
    if let Ok(secret) = std::env::var("APP_SECRET") {
        println!("cargo:rustc-env=APP_SECRET={}", secret);
    } else {
        println!("cargo:warning=APP_SECRET is not set in .env file");
    }

    tauri_build::build()
}
