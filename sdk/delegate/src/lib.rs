use proc_macro::TokenStream;

use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, ItemMod};

/// This macro attribute is used to inject instructions and struct needed from the delegation program.
///
/// Components can be delegate and undelegated to allow fast udpates in the Ephemeral Rollups.
///
/// # Example
/// ```ignore
///
/// pub fn delegate(ctx: Context<DelegateInput>) -> Result<()> {
///     let pda_seeds: &[&[u8]] = &[TEST_PDA_SEED];
///
///     let [payer, pda, owner_program, buffer, delegation_record, delegate_account_seeds, delegation_program, system_program] = [
///         &ctx.accounts.payer,
///         &ctx.accounts.pda,
///         &ctx.accounts.owner_program,
///         &ctx.accounts.buffer,
///         &ctx.accounts.delegation_record,
///         &ctx.accounts.delegate_account_seeds,
///         &ctx.accounts.delegation_program,
///         &ctx.accounts.system_program,
///     ];
///
///     delegate_account(
///         payer,
///         pda,
///         owner_program,
///         buffer,
///         delegation_record,
///         delegate_account_seeds,
///         delegation_program,
///         system_program,
///         pda_seeds,
///         0,
///         30000
///     )?;
///
///     Ok(())
/// }
///
/// #[derive(Accounts)]
/// pub struct DelegateInput<'info> {
///     pub payer: Signer<'info>,
///     /// CHECK: The pda to delegate
///     #[account(mut)]
///     pub pda: AccountInfo<'info>,
///     /// CHECK: The owner program of the pda
///     pub owner_program: AccountInfo<'info>,
///     /// CHECK: The buffer to temporarily store the pda data
///     #[account(mut)]
///     pub buffer: AccountInfo<'info>,
///     /// CHECK: The delegation record
///     #[account(mut)]
///     pub delegation_record: AccountInfo<'info>,
///     /// CHECK: The delegation account seeds
///     #[account(mut)]
///     pub delegate_account_seeds: AccountInfo<'info>,
///     /// CHECK: The delegation program
///     pub delegation_program: AccountInfo<'info>,
///     /// CHECK: The system program
///     pub system_program: AccountInfo<'info>,
/// }
/// ```
#[proc_macro_attribute]
pub fn delegate(_args: TokenStream, input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as syn::ItemMod);
    let modified = modify_component_module(ast);
    TokenStream::from(quote! {
        #modified
    })
}

/// Modifies the component module and adds the necessary functions and structs.
fn modify_component_module(mut module: ItemMod) -> ItemMod {
    let (imports, undelegate_fn, undelegate_struct) = generate_undelegate();
    module.content = module.content.map(|(brace, mut items)| {
        items.extend(
            vec![imports, undelegate_fn, undelegate_struct]
                .into_iter()
                .map(|item| {
                    syn::parse2(item).unwrap_or_else(|e| panic!("Failed to parse item: {}", e))
                })
                .collect::<Vec<_>>(),
        );
        (brace, items)
    });
    module
}

/// Generates the undelegate function and struct.
fn generate_undelegate() -> (TokenStream2, TokenStream2, TokenStream2) {
    (
        quote! {
            use delegation_program_sdk::undelegate_account;
        },
        quote! {
            #[automatically_derived]
            pub fn process_undelegation(ctx: Context<InitializeAfterUndelegation>, account_seeds: Vec<Vec<u8>>) -> Result<()> {
                let [delegated_account, buffer, payer, system_program] = [
                    &ctx.accounts.base_account,
                    &ctx.accounts.buffer,
                    &ctx.accounts.payer,
                    &ctx.accounts.system_program,
                ];
                undelegate_account(
                    delegated_account,
                    &id(),
                    buffer,
                    payer,
                    system_program,
                    account_seeds,
                )?;
                Ok(())
            }
        },
        quote! {
            #[automatically_derived]
            #[derive(Accounts)]
                pub struct InitializeAfterUndelegation<'info> {
                /// CHECK:`
                #[account(mut)]
                pub base_account: AccountInfo<'info>,
                /// CHECK:`
                #[account()]
                pub buffer: AccountInfo<'info>,
                /// CHECK:`
                #[account(mut)]
                pub payer: AccountInfo<'info>,
                /// CHECK:`
                pub system_program: AccountInfo<'info>,
            }
        },
    )
}
