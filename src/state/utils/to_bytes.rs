#[macro_export]
macro_rules! impl_to_bytes_without_discriminant_zero_copy {
    ($struct_name:ident) => {
        impl $struct_name {
            pub fn to_bytes_without_discriminant(&self) -> &[u8] {
                bytemuck::bytes_of(self)
            }
        }
    };
}
