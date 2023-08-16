use change_case::pascal_case;
use proc_macro2::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{parse_quote, Fields, Generics, Ident, ItemStruct, Path, Type, Visibility};

#[derive(Debug)]
pub enum ConnectType {
    Direct(Ident, Ident),
    Set(Ident, Ident),
}

pub fn get_source(input: &ItemStruct) -> Vec<ConnectType> {
    let Fields::Named(fields) = & input.fields else {panic!("Impossible!")};
    let mut result = Vec::new();
    let direct_path1: Path = parse_quote!(tgraph::typed_graph::NodeIndex);
    let direct_path2: Path = parse_quote!(typed_graph::NodeIndex);
    let set_path1: Path = parse_quote!(HashSet<NodeIndex>);
    let set_path2: Path = parse_quote!(std::collections::HashSet<NodeIndex>);
    let set_path3: Path = parse_quote!(collections::HashSet<NodeIndex>);
    for f in &fields.named {
        let ident = f.ident.clone().unwrap();
        if let Type::Path(p) = &f.ty {
            if p.path.is_ident("NodeIndex") || p.path == direct_path1 || p.path == direct_path2 {
                result.push(ConnectType::Direct(ident.clone(), upper_camel(&ident)))
            } else if p.path == set_path1 || p.path == set_path2 || p.path == set_path3 {
                result.push(ConnectType::Set(ident.clone(), upper_camel(&ident)))
            }
        }
    }
    result
}

pub fn make_enum(
    result: &mut TokenStream,
    sources: &Vec<ConnectType>,
    name: &Ident,
    vis: &Visibility,
) -> Ident {
    let source_enum = format_ident!("{}Source", name);
    let mut vars = Vec::new();
    for s in sources {
        match &s {
            ConnectType::Direct(_, camel) => vars.push(quote! {#camel}),
            ConnectType::Set(_, camel) => vars.push(quote! {#camel}),
        }
    }
    quote! {
        #[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
        #vis enum #source_enum{
            #(#vars),*
        }
    }
    .to_tokens(result);

    source_enum
}

pub fn make_iter(
    result: &mut TokenStream,
    sources: &Vec<ConnectType>,
    name: &Ident,
    vis: &Visibility,
    generics: &Generics,
    source_enum: &Ident,
) {
    let iterator_ident = format_ident!("{}SourceIterator", name);
    let mut add_source_ops = Vec::new();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    for s in sources {
        match s {
            ConnectType::Direct(ident, camel) => add_source_ops.push(quote! {
                sources.push((node.#ident, #source_enum::#camel));
            }),
            ConnectType::Set(ident, camel) => add_source_ops.push(quote! {
                for i in node.#ident.iter() {
                    sources.push((*i, #source_enum::#camel));
                }
            }),
        }
    }

    let mut modify_arms = Vec::new();
    for s in sources {
        modify_arms.push(match s {
            ConnectType::Direct(ident, camel) => quote! {
                #source_enum::#camel => self.#ident = new_idx,
            },
            ConnectType::Set(ident, camel) => quote! {
                #source_enum::#camel => {
                    self.#ident.remove(&old_idx);
                    self.#ident.insert(new_idx);
                },
            },
        })
    }
    quote! {
        #vis struct #iterator_ident {
            sources: Vec<(NodeIndex, #source_enum)>,
            cur: usize
        }
        impl #impl_generics tgraph::typed_graph::SourceIterator<#name #ty_generics> for #iterator_ident #where_clause{
            type Source = #source_enum;
            fn new(node: &#name #ty_generics) -> Self{
                let mut sources = Vec::new();
                #(#add_source_ops)*
                #iterator_ident{ sources, cur: 0 }
            }
        }
        impl std::iter::Iterator for #iterator_ident {
            type Item = (NodeIndex, #source_enum);
            fn next(&mut self) -> Option<Self::Item> {
                if self.cur == self.sources.len() {
                    None
                } else {
                    let result = self.sources[self.cur].clone();
                    self.cur += 1;
                    Some(result)
                }
            }
        }
        impl #impl_generics tgraph::typed_graph::TypedNode for #name #ty_generics #where_clause {
            type Source = #source_enum;
            type Iter = #iterator_ident;
            fn iter_source(&self) -> Self::Iter {
                #iterator_ident::new(&self)
            }
            fn modify(&mut self, source: Self::Source, old_idx:NodeIndex, new_idx: NodeIndex) {
                match source{
                    #(#modify_arms)*
                }
            }
        }
    }
    .to_tokens(result);
}

fn upper_camel(ident: &Ident) -> Ident {
    format_ident!("{}", pascal_case(&ident.to_string()))
}
