//! This module provides a DLS for writing[`Term`]s within Rust.
//!
//! # Example
//!
//! ```rust
//! use tlspuffin::tls::fn_impl::fn_client_hello;
//! use tlspuffin::term;
//! use tlspuffin::agent::AgentName;
//! use rustls::{ProtocolVersion, CipherSuite};
//! use rustls::msgs::handshake::{SessionID, Random, ClientExtension};
//! use rustls::msgs::enums::Compression;
//!
//! let client = AgentName::first();
//! let term = term! {
//!     fn_client_hello(
//!         ((client, 0)/ProtocolVersion),
//!         ((client, 0)/Random),
//!         ((client, 0)/SessionID),
//!         ((client, 0)/Vec<CipherSuite>),
//!         ((client, 0)/Vec<Compression>),
//!         ((client, 0)/Vec<ClientExtension>)
//!     )
//! };
//! ```

#[macro_export]
macro_rules! term {
    //
    // Handshake with TlsMessageType
    // `>$req_type:expr` must be the last part of the arm, even if it is not used.
    //
    (($agent:expr, $counter:expr) / $typ:ty $(>$req_type:expr)?) => {{
        use $crate::term::dynamic_function::TypeShape;

        // ignore $req_type as we are overriding it with $type
        term!(($agent, $counter) > TypeShape::of::<$typ>())
    }};
    (($agent:expr, $counter:expr) $(>$req_type:expr)?) => {{
        use $crate::trace::TlsMessageType;
        use $crate::term::signature::Signature;
        use $crate::term::Term;

        let var = Signature::new_var_by_type_id($($req_type)?, $agent, Some(TlsMessageType::Handshake(None)), $counter);
        Term::Variable(var)
    }};

    //
    // Handshake TlsMessageType with `$message_type` as `TlsMessageType`
    //
    (($agent:expr, $counter:expr) [$message_type:expr] / $typ:ty $(>$req_type:expr)?) => {{
        use $crate::term::dynamic_function::TypeShape;

        // ignore $req_type as we are overriding it with $type
        term!(($agent, $counter) [$message_type] > TypeShape::of::<$typ>())
    }};
    // Extended with custom $type
    (($agent:expr, $counter:expr) [$message_type:expr] $(>$req_type:expr)?) => {{
        use $crate::term::signature::Signature;
        use $crate::term::Term;

        let var = Signature::new_var_by_type_id($($req_type)?, $agent, $message_type, $counter);
        Term::Variable(var)
    }};

    //
    // Function Applications
    //
    ($func:ident ($($args:tt),*) $(>$req_type:expr)?) => {{
        use $crate::term::signature::Signature;
        use $crate::term::Term;

        let func = Signature::new_function(&$func);
        #[allow(unused_assignments, unused_variables, unused_mut)]
        let mut i = 0;

        #[allow(unused_assignments)]
        let arguments = vec![$({
            #[allow(unused)]
            let argument = func.shape().argument_types.get(i)
                    .expect("too many arguments specified for function")
                    .clone();
            i += 1;
            $crate::term_arg!($args > argument)
        }),*];

        Term::Application(func, arguments)
    }};
    // Shorthand for constants
    ($func:ident $(>$req_type:expr)?) => {{
        use $crate::term::signature::Signature;
        use $crate::term::Term;

        let func = Signature::new_function(&$func);
        Term::Application(func, vec![])
    }};

    //
    // Allows to use variables which already contain a term by starting with a `@`
    //
    (@$e:ident $(>$req_type:expr)?) => {{
        use $crate::term::Term;

        let subterm: &Term = &$e;
        subterm.clone()
    }};
}

#[macro_export]
macro_rules! term_arg {
    // Somehow the following rules is very important
    ( ( $($e:tt)* ) $(>$req_type:expr)?) => (term!($($e)* $(>$req_type)?));
    // not sure why I should need this
    // ( ( $e:tt ) ) => (ast!($e));
    ($e:tt $(>$req_type:expr)?) => (term!($e $(>$req_type)?));
}
