/// Defines a new actor type.
///
/// Use this macro to both define an actor's state type and wrapper type.
/// The wrapper type can later have `impl` blocks to add some convenient
/// methods to interact with the actor.
#[macro_export]
macro_rules! define_actor {
    {
        // The following special attributes must be specified:
        // - #[wrapper_type($wrapper_type:ident)]
        $(#[$($attrs:tt)*])*
        $v:vis struct $state_type:ident {
            $($state_items:tt)*
        }
    } => {
        $crate::__define_actor! {
            $(#[$($attrs)*])*
            $v struct $state_type {
                $($state_items)*
            }
        }
    }
}

#[doc(hidden)]
#[macro_export]
macro_rules! __define_actor {
    // Input
    {
        $(#[$($attrs:tt)*])*
        $v:vis struct $state_type:ident {
            $($state_items:tt)*
        }
    } => {
        $crate::__define_actor! {
            (
                $(#[$($attrs)*])*
            )

            ($v)
            ($state_type)
            ()
            ()

            {
                $($state_items)*
            }
        }
    };

    // Parse name attribute
    {
        (
            #[wrapper_type($parsed:ident)]
            $($rest:tt)*
        )

        ($v:vis)
        ($state_type:ident)
        ($($wrapper_type:tt)*)
        ($($doc:tt)*)

        {
            $($state_items:tt)*
        }
    } => {
        $crate::__define_actor! {
            (
                $($rest)*
            )

            ($v)
            ($state_type)
            ($parsed)
            ($($doc)*)

            {
                $($state_items)*
            }
        }
    };

    // Parse doc attribute
    {
        (
            #[doc = $($parsed:tt)*]
            $($rest:tt)*
        )

        ($v:vis)
        ($state_type:ident)
        ($($wrapper_type:tt)*)
        ($($doc:tt)*)

        {
            $($state_items:tt)*
        }
    } => {
        $crate::__define_actor! {
            (
                $($rest)*
            )

            ($v)
            ($state_type)
            ($($wrapper_type)*)
            ($($doc)* #[doc = $($parsed)*])

            {
                $($state_items)*
            }
        }
    };

    // Validate attributes and forward to output
    {
        (
            $($rest:tt)*
        )

        ($v:vis)
        ($state_type:ident)
        ($wrapper_type:ident)
        ($($doc:tt)*)

        {
            $($state_items:tt)*
        }
    } => {
        $crate::__define_actor! {
            @
            (
                $($rest)*
            )

            ($v)
            ($state_type)
            ($wrapper_type)
            ($($doc)*)

            {
                $($state_items)*
            }
        }
    };
    {
        (
            $($rest:tt)*
        )

        ($v:vis)
        ($state_type:ident)
        ()
        ($($doc:tt)*)

        {
            $($state_items:tt)*
        }
    } => {
        compile_error!("Wrapper type must be specified")
    };

    // Output
    {
        @
        (
            $(#[$($rest_attrs:tt)*])*
        )

        ($v:vis)
        ($state_type:ident)
        ($wrapper_type:ident)
        ($($doc:tt)*)

        {
            $($state_items:tt)*
        }
    } => {
        $(#[$($rest_attrs)*])*
        struct $state_type {
            $($state_items)*
        }

        $($doc)*
        $v struct $wrapper_type {
            handle: $crate::Actor<$state_type>
        }

        impl $wrapper_type {
            #[inline]
            fn spawn(
                state: $state_type,
                label: Option<&str>
            ) -> $wrapper_type {
                let handle = $crate::Actor::spawn(state, label);
                $wrapper_type {
                    handle
                }
            }

            #[inline]
            fn handle(&self) -> &$crate::Actor<$state_type> {
                &self.handle
            }
        }

        impl Clone for $wrapper_type {
            fn clone(&self) -> Self {
                $wrapper_type {
                    handle: self.handle.clone()
                }
            }
        }
    };
}
