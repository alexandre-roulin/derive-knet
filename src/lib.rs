extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(DeriveKnet)]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    
    let ienum = &ast.ident;
    let ienum_string = ienum.to_string();

    //Iterator enum
    let variants = if let syn::Data::Enum(syn::DataEnum { variants, .. }) = &ast.data {
        variants
    } else {
        unimplemented!("Derive Knet only support Enum")
    };

    //Max len of identifier
    let max_ident = variants
        .iter()
        .map(|v| v.ident.to_string().len())
        .max_by(|v1, v2| v1.cmp(&v2));

    //match for serialize()
    let serialize_variants = variants.iter().map(|v| {
        let name = &v.ident;
        let sname = name.to_string();
        let ty = inner_ty(&v.fields);
        quote! {
            #ienum::#name(data) => {
                let size = ::std::mem::size_of::<#ty>();
                let mut v  = vec![0u8; #max_ident + size];
                let src_ptr = ::std::boxed::Box::new(*data);
                let dst_ptr = v.as_mut_ptr();
                /// SAFETY : TODO
                unsafe {
                    std::ptr::copy_nonoverlapping(#sname.as_ptr(), dst_ptr, #sname.len());
                    let dst_ptr = dst_ptr.offset(#max_ident as isize);
                    let src_ptr = ::std::boxed::Box::into_raw(src_ptr).cast::<u8>();
                    std::ptr::copy_nonoverlapping(src_ptr, dst_ptr, size);
                };
                let _ = ::std::boxed::Box::from_raw(src_ptr);

                v
            }
        }
    });

    //function serialize()
    let serialize = quote! {
        fn serialize(&self) -> ::std::vec::Vec<u8> {
            match self {
                #(#serialize_variants)*
            }
        }
    };

    //match for deserialize()
    let deserialize_variants = variants.iter().map(|v| {
        let name = &v.ident;
        let sname = name.to_string();
        let ty = inner_ty(&v.fields).unwrap();
        quote!{
            #sname => {
                let mut data = #ty::default();
                let src_ptr = v[#max_ident..].as_ptr();
                let dst_ptr = ::std::boxed::Box::into_raw(::std::boxed::Box::new(data)).cast::<u8>();
                ///SAFETY : TODO
                unsafe {
                    std::ptr::copy_nonoverlapping(src_ptr, dst_ptr, ::std::mem::size_of::<#ty>());
                    data = *dst_ptr.cast::<#ty>();
                    ::std::boxed::Box::from_raw(dst_ptr);
                };
                *self = #ienum::#name(data);
            }
        }
    });

    //deserialize function
    let deserialize = quote! {
        fn deserialize(&mut self, v: &[u8]) {
            let name = std::string::String::from_utf8(v[0..#max_ident].to_vec())
            .unwrap();
            let name = name.trim_end_matches(|c| c as u8 == 0u8);

            match name {
                #(#deserialize_variants),*
                _ => { panic!("`{}` is not a part of {}", name, #ienum_string) }
            }
        }
    };

    //match for from_raw
    let from_raw_variants = variants.iter().map(|v| {
        let name = &v.ident;
        let sname = name.to_string();
        let ty = inner_ty(&v.fields).unwrap();
        quote! {
            #sname => {
                let mut data = #ty::default();
                let src_ptr = v[#max_ident..].as_ptr();
                let dst_ptr = ::std::boxed::Box::into_raw(::std::boxed::Box::new(data)).cast::<u8>();
                ///SAFETY : TODO
                unsafe {
                    std::ptr::copy_nonoverlapping(src_ptr, dst_ptr, ::std::mem::size_of::<#ty>());
                    data = *dst_ptr.cast::<#ty>();
                    ::std::boxed::Box::from_raw(dst_ptr);
                };
                ///SAFETY : TODO
                #ienum::#name(data)
            }
        }
    });

    //function from_raw
    let from_raw = quote! {
        fn from_raw(v: &[u8]) -> Self {
            let name = std::string::String::from_utf8(v[0..#max_ident].to_vec())
                .unwrap();
            let name = name.trim_end_matches(|c| c as u8 == 0u8);

            match name {
                #(#from_raw_variants),*
                _ => { panic!("`{}` is not a part of {}", name, #ienum_string) }
            }
        }

    };

    
    let get_size_variants = variants.iter().map(|v| {
        let name = &v.ident;
        let sname = name.to_string();
        let ty = inner_ty(&v.fields).unwrap();
        quote! {
            #sname => {
                ::std::mem::size_of::<#ty>()
            }
        }
    });

    let get_size_of_data = quote! {
        fn get_size_of_data(v: &[u8]) -> usize {
            let name = std::string::String::from_utf8(v.to_vec())
                .unwrap();
            let name = name.trim_end_matches(|c| c as u8 == 0u8);
            match name {
                #(#get_size_variants),*
                _ => { panic!("`{}` is not a part of {}", name, #ienum_string) }

            }
        }
    };

    let get_size_of_payload = quote! {
        fn get_size_of_payload() -> usize {
            #max_ident
        }
    };
    let expanded = quote! {
        impl knet::KnetTransform for #ienum {
            #serialize
            #deserialize
            #from_raw
            #get_size_of_data
            #get_size_of_payload
        }
    };
    expanded.into()
}

fn inner_ty(field: &syn::Fields) -> Option<&syn::Ident> {
    if let syn::Fields::Unnamed(syn::FieldsUnnamed { unnamed, .. }) = field {
        if let Some(first) = unnamed.first() {
            if let syn::Type::Path(syn::TypePath { path, .. }) = &first.into_value().ty {
                if path.segments.len() == 1 {
                    return Some(&path.segments[0].ident);
                }
            }
        }
    }
    None
}