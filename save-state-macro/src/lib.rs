use proc_macro::TokenStream;

struct ParseExprList([syn::Expr; 2]);

impl syn::parse::Parse for ParseExprList {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::parse::Result<Self> {
        let elems =
            syn::punctuated::Punctuated::<syn::Expr, syn::Token!(,)>::parse_terminated(input)?;
        if elems.len() != 2 {
            Err(syn::parse::Error::new(
                input.span(),
                format_args!("needing 2 elements in attribute, got {}", elems.len()),
            ))
        } else {
            Ok(Self([
                elems.first().unwrap().clone(),
                elems.last().unwrap().clone(),
            ]))
        }
    }
}

fn get_struct_fields(
    struct_fields: &syn::Fields,
) -> (Vec<impl quote::ToTokens>, Vec<impl quote::ToTokens>) {
    let fields: Vec<_> = struct_fields
        .iter()
        .map(|field| {
            if let Some(attr) = field.attrs.iter().find(|attr| {
                attr.path
                    .segments
                    .last()
                    .filter(|i| i.ident.to_string() == "except")
                    .is_some()
            }) {
                (Some(attr.parse_args::<ParseExprList>().unwrap().0), field)
            } else {
                (None, field)
            }
        })
        .collect();
    let (ser_expr, deser_expr) = (
        fields
            .iter()
            .enumerate()
            .map(|(i, (ser_deser, field))| {
                let field_name = &field.ident;
                let i = syn::Index::from(i);
                if let Some(field_name) = field_name {
                    if let Some([ser, _deser]) = ser_deser {
                        quote::quote! {{
                            let f = (#ser);
                            let state: &mut save_state::SaveStateSerializer = state;
                            let _: () = f(&self.#field_name, state);
                        }}
                    } else {
                        quote::quote! {
                            self.#field_name.serialize(state)
                        }
                    }
                } else {
                    if let Some([ser, _deser]) = ser_deser {
                        quote::quote! {{
                            let f = (#ser);
                            let state: &mut save_state::SaveStateSerializer = state;
                            let _: () = f(&self.#i, state);
                        }}
                    } else {
                        quote::quote! {
                            self.#i.serialize(state)
                        }
                    }
                }
            })
            .collect::<Vec<_>>(),
        fields
            .iter()
            .enumerate()
            .map(|(i, (ser_deser, field))| {
                let field_name = &field.ident;
                let i = syn::Index::from(i);
                if let Some(field_name) = field_name {
                    if let Some([_ser, deser]) = ser_deser {
                        quote::quote! {{
                            let f = (#deser);
                            let state: &mut save_state::SaveStateDeserializer = state;
                            let _: () = f(&mut self.#field_name, state);
                        }}
                    } else {
                        quote::quote! {
                            self.#field_name.deserialize(state)
                        }
                    }
                } else {
                    if let Some([_ser, deser]) = ser_deser {
                        quote::quote! {{
                            let f = (#deser);
                            let state: &mut save_state::SaveStateDeserializer = state;
                            let _: () = f(&mut self.#i, state)
                        }}
                    } else {
                        quote::quote! {
                            self.#i.deserialize(state)
                        }
                    }
                }
            })
            .collect::<Vec<_>>(),
    );
    (ser_expr, deser_expr)
}

#[proc_macro_derive(InSaveState, attributes(except))]
pub fn derive_in_save_state(input_struct: TokenStream) -> TokenStream {
    match syn::parse::<syn::DeriveInput>(input_struct.clone()) {
        Ok(derive_input) => {
            let (impl_generics, ty_generics, where_clause) = derive_input.generics.split_for_impl();
            let ty_name = &derive_input.ident;
            let (ser_expr, deser_expr) = match derive_input.data {
                syn::Data::Struct(field_struct) => get_struct_fields(&field_struct.fields),
                _ => {
                    return {
                        let text = format!("expected struct, got `{}`", derive_input.ident);
                        syn::parse::Error::new_spanned(derive_input, text)
                    }
                    .into_compile_error()
                    .into()
                }
            };
            quote::quote!(
                impl #impl_generics save_state::InSaveState
                        for #ty_name #ty_generics #where_clause {
                    fn serialize(&self, state: &mut save_state::SaveStateSerializer) {
                        #(#ser_expr;)*
                    }

                    fn deserialize(&mut self, state: &mut save_state::SaveStateDeserializer) {
                        #(#deser_expr;)*
                    }
                }
            )
            .into()
        }
        Err(err) => err.to_compile_error().into(),
    }
}

#[proc_macro_derive(DefaultByNew)]
pub fn derive_default_by_new(input_struct: TokenStream) -> TokenStream {
    let derive_input = match syn::parse::<syn::DeriveInput>(input_struct.clone()) {
        Ok(derive_input) => derive_input,
        Err(err) => return err.to_compile_error().into(),
    };
    let (impl_generics, ty_generics, where_clause) = derive_input.generics.split_for_impl();
    let ty_name = &derive_input.ident;
    quote::quote! {
        impl #impl_generics Default for #ty_name #ty_generics #where_clause {
            fn default() -> Self {
                Self::new()
            }
        }
    }
    .into()
}
