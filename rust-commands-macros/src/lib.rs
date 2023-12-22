use darling::FromField;
use syn::{DeriveInput, parse_macro_input, Ident, Type};
use quote::quote;

#[derive(Debug, FromField)]
#[darling(attributes(marshal))]
struct StructFieldReceiver {
  ident: Option<Ident>,
  ty: Type,
}

#[proc_macro_derive(Systems, attributes(system))]
pub fn derive_systems(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
  let DeriveInput {
    attrs, vis: _, ident, generics: _, data
  } = parse_macro_input!(input as DeriveInput);

  match data {
    syn::Data::Struct(st) => {
      match st.fields {
        syn::Fields::Named(fields) => {
          let shared_ident = syn::Ident::new(&format!("{}Shared", ident), ident.span());

          let mapped_fields = fields.named.iter().map(|field| StructFieldReceiver::from_field(field).unwrap());

          let shared_fields = mapped_fields.clone().map(|field| {
            let StructFieldReceiver {
              ident: f_ident, ty: f_ty
            } = field;

            quote! {
              #f_ident: std::sync::Arc<rust_commands::System<#f_ty>>
            }
          });

          let field_initialisers = mapped_fields.clone().map(|field| {
            let StructFieldReceiver {
              ident: f_ident, ty: _
            } = field;

            quote! {
              #f_ident: std::sync::Arc::new(rust_commands::System::new(self.#f_ident))
            }
          });

          quote! {
            struct #shared_ident {
              #(#shared_fields),*
            }

            impl rust_commands::Systems for #ident {
              type Shared = #shared_ident;
              fn shared(self) -> #shared_ident {
                #shared_ident {
                  #(#field_initialisers),*
                }
              }
            }
          }.into()
        },
        _ => panic!("Non-Named Structs can't be used for commands.")
      }
    },
    _ => panic!("Enums can't be used for commands.")
  }
}
