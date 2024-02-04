use config_struct::{Error, FloatSize, IntSize, SerdeSupport, StructOptions};
use config_struct::DynamicLoading::DebugOnly;

fn main() -> Result<(), Error> {

    let server_options = StructOptions {
        format: Default::default(),
        struct_name: "ServerConfig".to_string(),
        const_name: Option::from("SERVER_CONFIG".to_string()),
        generate_const: false,
        derived_traits: Vec::from(["Debug".to_string(), "Clone".to_string()]),
        serde_support: SerdeSupport::Yes,
        use_serde_derive_crate: false,
        generate_load_fns: true,
        dynamic_loading: DebugOnly,
        create_dirs: true,
        write_only_if_changed: false,
        default_float_size: FloatSize::F64,
        default_int_size: IntSize::I64,
        max_array_size: 0,
    };

    config_struct::create_struct(
        "config/server.yaml",
        "src/server_config.rs",
        &server_options)




}