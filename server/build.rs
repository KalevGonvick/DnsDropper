use config_struct::{DynamicLoading, Error, FloatSize, Format, IntSize, SerdeSupport, StructOptions};

fn main() -> Result<(), Error> {

    let internal_options = StructOptions {
        format: Option::from(Format::Yaml),
        struct_name: "InternalConfig".to_string(),
        const_name: Option::from("INTERNAL_CONFIG".to_string()),
        ..Default::default()
    };

    config_struct::create_struct(
        "template/internal.yaml",
        "src/config/internal.rs",
        &internal_options).unwrap();

    let server_options = StructOptions {
        format: Option::from(Format::Yaml),
        struct_name: "ServerConfig".to_string(),
        const_name: Option::from("SERVER_CONFIG".to_string()),
        generate_const: false,
        derived_traits: Vec::from(["Debug".to_string(), "Clone".to_string()]),
        serde_support: SerdeSupport::Yes,
        use_serde_derive_crate: false,
        generate_load_fns: true,
        dynamic_loading: DynamicLoading::Always,
        create_dirs: true,
        write_only_if_changed: false,
        default_float_size: FloatSize::F64,
        default_int_size: IntSize::I64,
        max_array_size: 0,
    };

    config_struct::create_struct(
        "template/server-config.template.yaml",
        "src/config/server_config.rs",
        &server_options)
}