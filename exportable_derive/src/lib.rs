// exportable_derive/src/lib.rs

extern crate proc_macro;

use heck::ToSnakeCase;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Data, DataStruct, DeriveInput, Fields, GenericArgument, Ident, LitStr, PathArguments, Type,
    parenthesized, parse_macro_input,
};

#[proc_macro_derive(PrometheusExportable, attributes(prometheus))]
pub fn prometheus_exportable_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let struct_name = &input.ident; // e.g., Unit, IOStats
    let struct_name_snake = struct_name.to_string().to_snake_case(); // e.g., unit, iostats
    let metrics_holder_name = format_ident!("{}MetricsHolder", struct_name); // e.g., UnitMetricsHolder

    let fields = if let Data::Struct(DataStruct {
        fields: Fields::Named(fields),
        ..
    }) = input.data
    {
        fields.named
    } else {
        return syn::Error::new_spanned(
            input.ident,
            "PrometheusExportable can only be derived for structs with named fields.",
        )
        .to_compile_error()
        .into();
    };

    let mut metrics_holder_declarations = Vec::new(); // Fields for the MetricsHolder struct
    let mut metrics_holder_initializations = Vec::new(); // Code to init MetricsHolder fields
    let mut export_metrics_statements = Vec::new(); // Code for the export_metrics function

    for field in fields.iter() {
        let field_name = field.ident.as_ref().unwrap(); // e.g., read_bytes, active_state
        let field_type = &field.ty; // e.g., u64, String, Option<IOStats>

        // Parse #[prometheus(...)] attributes
        let mut description = String::new();
        let mut is_ignored = false;
        let mut is_counter = false;

        for attr in &field.attrs {
            if attr.path().is_ident("prometheus") {
                let result = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("description") {
                        let content;
                        parenthesized!(content in meta.input);
                        let lit: LitStr = content.parse()?;
                        description = lit.value();
                    } else if meta.path.is_ident("ignore") {
                        is_ignored = true;
                    } else if meta.path.is_ident("counter") {
                        is_counter = true;
                    } else {
                        eprintln!("Warning: Unrecognized attribute inside #[prometheus(...)] on field {}: {:?}", field_name, meta.path);
                    }
                    Ok(())
                });
                if let Err(err) = result {
                    eprintln!(
                        "Warning: Failed to parse prometheus attribute on field {}: {:#}",
                        field_name, err
                    );
                }
            }
        }

        if is_ignored {
            continue; // Skip this field entirely
        }

        // Generate metric name: struct_name_snake_field_name_snake
        let prometheus_metric_name = format!(
            "{}_{}",
            struct_name_snake,
            field_name.to_string().to_snake_case()
        );
        let metric_field_ident = format_ident!("{}_metric", prometheus_metric_name); // Field in MetricsHolder

        // Determine the metric description. If empty, create a default one.
        let actual_description = if description.is_empty() {
            format!(
                "Metrics for {}_{}.",
                struct_name_snake,
                field_name.to_string().to_snake_case()
            )
        } else {
            description.clone() // Clone description to use it in quote! block
        };

        match field_type {
            Type::Path(type_path) if type_path.path.is_ident("String") => {
                // String fields become a gauge with the string as a label and value 1
                let label_key =
                    Ident::new(&field_name.to_string().to_snake_case(), field_name.span());

                metrics_holder_declarations.push(quote! {
                    #metric_field_ident: prometheus_client::metrics::gauge::Gauge,
                });
                metrics_holder_initializations.push(quote! {
                    #metric_field_ident: {
                        let gauge = prometheus_client::metrics::gauge::Gauge::default();
                        registry.register(
                            #prometheus_metric_name,
                            #actual_description, // Use actual_description here
                            gauge.clone(),
                        );
                        gauge
                    },
                });
                export_metrics_statements.push(quote! {
                    let mut labels = accumulated_labels.clone(); // Clone for this specific metric
                    labels.insert(String::from(stringify!(#label_key)), self.#field_name.clone());
                    metrics_holder.#metric_field_ident.get_or_create(&labels).set(1);
                });
            }
            Type::Path(type_path)
                if type_path.path.is_ident("u64")
                    || type_path.path.is_ident("u32")
                    || type_path.path.is_ident("i64")
                    || type_path.path.is_ident("i32") =>
            {
                if is_counter {
                    // Numeric fields with #[prometheus(counter)] become Counters
                    metrics_holder_declarations.push(quote! {
                        #metric_field_ident: prometheus_client::metrics::counter::Counter,
                    });
                    metrics_holder_initializations.push(quote! {
                        #metric_field_ident: {
                            let counter = prometheus_client::metrics::counter::Counter::default();
                            registry.register(
                                #prometheus_metric_name,
                                #actual_description, // Use actual_description here
                                counter.clone(),
                            );
                            counter
                        },
                    });
                    export_metrics_statements.push(quote! {
                        metrics_holder.#metric_field_ident.get_or_create(accumulated_labels).inc_by(self.#field_name as u64);
                    });
                } else {
                    // Numeric fields without #[prometheus(counter)] become Gauges
                    metrics_holder_declarations.push(quote! {
                        #metric_field_ident: prometheus_client::metrics::gauge::Gauge,
                    });
                    metrics_holder_initializations.push(quote! {
                        #metric_field_ident: {
                            let gauge = prometheus_client::metrics::gauge::Gauge::default();
                            registry.register(
                                #prometheus_metric_name,
                                #actual_description, // Use actual_description here
                                gauge.clone(),
                            );
                            gauge
                        },
                    });
                    export_metrics_statements.push(quote! {
                        metrics_holder.#metric_field_ident.get_or_create(accumulated_labels).set(self.#field_name as i64);
                    });
                }
            }
            Type::Path(type_path) if type_path.path.is_ident("bool") => {
                // Boolean fields as 0 or 1 gauge
                metrics_holder_declarations.push(quote! {
                    #metric_field_ident: prometheus_client::metrics::gauge::Gauge,
                });
                metrics_holder_initializations.push(quote! {
                    #metric_field_ident: {
                        let gauge = prometheus_client::metrics::gauge::Gauge::default();
                        registry.register(
                            #prometheus_metric_name,
                            #actual_description, // Use actual_description here
                            gauge.clone(),
                        );
                        gauge
                    },
                });
                export_metrics_statements.push(quote! {
                    metrics_holder.#metric_field_ident.get_or_create(accumulated_labels).set(if self.#field_name { 1 } else { 0 });
                });
            }
            Type::Path(type_path) if type_path.path.segments.last().unwrap().ident == "Option" => {
                // Handle Option<T> for potential nested PrometheusExportable structs
                if let PathArguments::AngleBracketed(args) =
                    &type_path.path.segments.last().unwrap().arguments
                {
                    if let Some(GenericArgument::Type(inner_type)) = args.args.first() {
                        // Check if inner_type could be a PrometheusExportable struct
                        // We assume any named struct type inside Option<T> could be exportable
                        if let Type::Path(inner_type_path) = inner_type {
                            let inner_struct_name =
                                &inner_type_path.path.segments.last().unwrap().ident;
                            let inner_metrics_holder_name =
                                format_ident!("{}MetricsHolder", inner_struct_name);
                            let inner_metrics_field =
                                format_ident!("{}_metrics_holder", field_name);

                            metrics_holder_declarations.push(quote! {
                                #inner_metrics_field: #inner_metrics_holder_name,
                            });
                            metrics_holder_initializations.push(quote! {
                                #inner_metrics_field: #inner_metrics_holder_name::register_metric_families(registry),
                            });
                            export_metrics_statements.push(quote! {
                                if let Some(inner_instance) = &self.#field_name {
                                    let mut nested_labels = accumulated_labels.clone(); // Clone for nested call
                                    inner_instance.export_metrics(&metrics_holder.#inner_metrics_field, &mut nested_labels);
                                }
                            });
                        }
                    }
                }
            }
            Type::Path(type_path) => {
                // Assume other non-Option path types are also PrometheusExportable for recursion
                let inner_struct_name = &type_path.path.segments.last().unwrap().ident;
                let inner_metrics_holder_name = format_ident!("{}MetricsHolder", inner_struct_name);
                let inner_metrics_field = format_ident!("{}_metrics_holder", field_name);

                metrics_holder_declarations.push(quote! {
                    #inner_metrics_field: #inner_metrics_holder_name,
                });
                metrics_holder_initializations.push(quote! {
                    #inner_metrics_field: #inner_metrics_holder_name::register_metric_families(registry),
                });
                export_metrics_statements.push(quote! {
                    let mut nested_labels = accumulated_labels.clone(); // Clone for nested call
                    self.#field_name.export_metrics(&metrics_holder.#inner_metrics_field, &mut nested_labels);
                });
            }
            _ => {
                // Ignore other types for now, or add more specific handling
                eprintln!(
                    "Warning: Field '{}' of type '{:?}' is not yet handled by PrometheusExportable macro.",
                    field_name, field_type
                );
            }
        }
    }

    let expanded = quote! {
        // Defines the struct that holds all the metric families for `struct_name`
        #[allow(non_snake_case)] // Allows generated field names like `iostats_read_bytes_metric`
        pub struct #metrics_holder_name {
            #(#metrics_holder_declarations)*
        }

        impl #metrics_holder_name {
            /// Registers all Prometheus metric families for `#struct_name` and its nested `PrometheusExportable` types.
            pub fn register_metric_families(registry: &mut prometheus_client::registry::Registry) -> Self {
                use prometheus_client::metrics::gauge::Gauge;
                use prometheus_client::metrics::counter::Counter; // Import Counter
                use std::collections::BTreeMap; // Use BTreeMap for labels

                Self {
                    #(#metrics_holder_initializations)*
                }
            }
        }

        // Implements the PrometheusExportable trait for `struct_name`
        impl self::PrometheusExportable for #struct_name {
            type MetricsHolder = #metrics_holder_name;

            fn register_metric_families(registry: &mut prometheus_client::registry::Registry) -> Self::MetricsHolder {
                #metrics_holder_name::register_metric_families(registry)
            }

            fn export_metrics(
                &self,
                metrics_holder: &Self::MetricsHolder,
                // `accumulated_labels` are passed down from parent structs (e.g., name/machine from Unit)
                // This allows nested metrics to carry parent context.
                accumulated_labels: &mut std::collections::BTreeMap<String, String>,
            ) {
                use prometheus_client::metrics::gauge::Gauge;
                use prometheus_client::metrics::counter::Counter; // Import Counter
                use std::collections::BTreeMap;

                // Add struct-specific identifying labels if needed (e.g., `name` and `machine` for `Unit`)
                // This part needs to be specifically handled by the user or by a more advanced attribute,
                // so for this example, we assume `name` and `machine` are already in `accumulated_labels`
                // if this is the top-level Unit.
                // For nested structs like IOStats, they simply use the labels passed to them.

                #(#export_metrics_statements)*
            }
        }
    };

    expanded.into()
}
