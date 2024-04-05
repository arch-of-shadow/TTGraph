use std::collections::{btree_map, BTreeMap, BTreeSet, HashMap};

use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{self, custom_punctuation, parse2, Ident, Token, Type};

use crate::utils::*;

custom_punctuation!(BidirectionalSep, <->);

pub(crate) struct BidirectionalLink {
  pub var1: Ident,
  pub link1: Ident,
  pub var2: Ident,
  pub link2: Ident,
}

impl Parse for BidirectionalLink {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    Ok({
      let var1: Ident = input.parse()?;
      let _: Token![.] = input.parse()?;
      let link1: Ident = input.parse()?;
      let _: BidirectionalSep = input.parse()?;
      let var2: Ident = input.parse()?;
      let _: Token![.] = input.parse()?;
      let link2: Ident = input.parse()?;
      BidirectionalLink { var1, link1, var2, link2 }
    })
  }
}

struct BidirectionalLinkVec {
  links: Vec<BidirectionalLink>,
}

impl Parse for BidirectionalLinkVec {
  fn parse(input: ParseStream) -> syn::Result<Self> {
    let mut links = Vec::new();

    while !input.is_empty() {
      let lookahead = input.lookahead1();
      if lookahead.peek(Token![,]) {
        let _: Token![,] = input.parse()?;
      } else {
        links.push(input.parse()?);
      }
    }

    Ok(BidirectionalLinkVec { links })
  }
}

pub(crate) fn get_bidiretional(
  tokens: TokenStream, links: &mut Vec<BidirectionalLink>,
) -> syn::Result<()> {
  let link_vec: BidirectionalLinkVec = parse2(tokens)?;
  links.extend(link_vec.links.into_iter());
  Ok(())
}

pub(crate) fn make_bidirectional_link(
  vars: &Vec<(Ident, Type)>, links: &Vec<BidirectionalLink>,
) -> TokenStream {
  let mut b_links: BTreeMap<Ident, BTreeMap<Ident, BTreeSet<(Ident, Ident)>>> =
    BTreeMap::new();
  let mut ty_map: HashMap<Ident, Type> = HashMap::new();

  for (var, ty) in vars {
    ty_map.insert(var.clone(), ty.clone());
  }

  for link in links {
    b_links
      .entry(link.var1.clone())
      .or_default()
      .entry(link.link1.clone())
      .or_default()
      .insert((link.var2.clone(), link.link2.clone()));
    b_links
      .entry(link.var2.clone())
      .or_default()
      .entry(link.link2.clone())
      .or_default()
      .insert((link.var1.clone(), link.link1.clone()));
  }

  let mut link_mirrors_of_arms = Vec::new();
  for (var, ty) in vars {
    if let btree_map::Entry::Occupied(v) = b_links.entry(var.clone()) {
      let mut arms = Vec::new();
      for (link, to) in v.get() {
        let camel = upper_camel(link);
        let mut possible_links = Vec::new();
        for (var2, link2) in to {
          let var2_ty = &ty_map[var2];
          let link2_camel = upper_camel(link2);
          possible_links.push(quote!{
              Self::LinkMirrorEnum::#var2(<#var2_ty as tgraph::typed_graph::TypedNode>::LinkMirror::#link2_camel)
            });
        }
        arms.push(quote!{
            <#ty as tgraph::typed_graph::TypedNode>::LinkMirror::#camel => vec![#(#possible_links),*],
          });
      }
      link_mirrors_of_arms.push(quote! {
        Self::LinkMirrorEnum::#var(l) => {
          match l {
            #(#arms)*
          }
        }
      });
    }
  }

  let mut links_arms = Vec::new();
  for (var, ty) in vars {
    if let btree_map::Entry::Occupied(v) = b_links.entry(var.clone()) {
      let mut vecs = Vec::new();
      for (link, _) in v.get() {
        let camel = upper_camel(link);
        vecs.push(quote!{
            (
              Vec::from_iter(self.iter_link(Self::LinkMirrorEnum::#var(<#ty as tgraph::typed_graph::TypedNode>::LinkMirror::#camel))),
              self.get_bidiretional_link_mirrors_of(Self::LinkMirrorEnum::#var(<#ty as tgraph::typed_graph::TypedNode>::LinkMirror::#camel)),
            ),
          })
      }

      links_arms.push(quote! {
        Self::#var(x) => {
          vec![#(#vecs)*]
        },
      });
    }
  }

  quote! {
    fn get_bidiretional_links(&self) -> tgraph::typed_graph::BidirectionalLinks<Self::LinkMirrorEnum> {
      match self {
        #(#links_arms)*
        _ => vec![],
      }
    }
    fn get_bidiretional_link_mirrors_of(&self, link: Self::LinkMirrorEnum) -> Vec<Self::LinkMirrorEnum> {
      match link {
        #(#link_mirrors_of_arms)*
        _ => vec![],
      }
    }
  }
}
