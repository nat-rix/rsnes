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

#[proc_macro_derive(InSaveState, attributes(except))]
pub fn derive_in_save_state(input_struct: TokenStream) -> TokenStream {
    match syn::parse::<syn::DeriveInput>(input_struct.clone()) {
        Ok(derive_input) => {
            let (impl_generics, ty_generics, where_clause) = derive_input.generics.split_for_impl();
            let ty_name = &derive_input.ident;
            let (ser_expr, deser_expr) = match derive_input.data {
                syn::Data::Struct(field_struct) => {
                    let fields: Vec<_> = field_struct
                        .fields
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
                            .map(|(ser_deser, field)| {
                                let field_name = &field.ident;
                                if let Some([ser, _deser]) = ser_deser {
                                    quote::quote! {{
                                        let f = (#ser);
                                        let state: &mut save_state::SaveStateSerializer = state;
                                        let _: () = f(self, state);
                                    }}
                                } else {
                                    quote::quote! {
                                        self.#field_name.serialize(state)
                                    }
                                }
                            })
                            .collect::<Vec<_>>(),
                        fields
                            .iter()
                            .map(|(ser_deser, field)| {
                                let field_name = &field.ident;
                                let field_ty = &field.ty;
                                if let Some([_ser, deser]) = ser_deser {
                                    quote::quote! {
                                        let #field_name = {
                                            let f = (#deser);
                                            let state: &mut save_state::SaveStateDeserializer = state;
                                            let v: Option<_> = f(state);
                                            v?
                                        }
                                    }
                                } else {
                                    quote::quote! {
                                        let #field_name = <#field_ty>::deserialize(state)?
                                    }
                                }
                            })
                            .collect::<Vec<_>>(),
                    );
                    let field_names = fields.iter().map(|(_, f)| &f.ident);
                    let deser_expr = quote::quote! {{
                        #(#deser_expr;)* return Some(Self { #(#field_names,)* });
                    }};
                    (ser_expr, deser_expr)
                }
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
                impl #impl_generics save_state::InSaveState for #ty_name #ty_generics
                        #where_clause {
                    fn serialize(&self, state: &mut save_state::SaveStateSerializer) {
                        #(#ser_expr;)*
                    }

                    fn deserialize(state: &mut save_state::SaveStateDeserializer) -> Option<Self> {
                        #deser_expr
                    }
                }
            )
            .into()
        }
        Err(err) => err.to_compile_error().into(),
    }
}
