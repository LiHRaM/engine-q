use nu_engine::CallExt;
use nu_protocol::ast::{Call, CellPath};
use nu_protocol::engine::{Command, EngineState, Stack};
use nu_protocol::{Example, PipelineData, ShellError, Signature, Span, SyntaxShape, Value};
use std::marker::PhantomData;

pub trait HashDigest: digest::Digest + Clone {
    fn name() -> &'static str;
    fn examples() -> Vec<Example>;
}

#[derive(Clone)]
pub struct GenericDigest<D: HashDigest> {
    name: String,
    usage: String,
    phantom: PhantomData<D>,
}

impl<D: HashDigest> Default for GenericDigest<D> {
    fn default() -> Self {
        Self {
            name: format!("hash {}", D::name()),
            usage: format!("hash a value using the {} hash algorithm", D::name()),
            phantom: PhantomData,
        }
    }
}

impl<D> Command for GenericDigest<D>
where
    D: HashDigest + Send + Sync + 'static,
    digest::Output<D>: core::fmt::LowerHex,
{
    fn name(&self) -> &str {
        &self.name
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name()).rest(
            "rest",
            SyntaxShape::CellPath,
            format!("optionally {} hash data by cell path", D::name()),
        )
    }

    fn usage(&self) -> &str {
        &self.usage
    }

    fn examples(&self) -> Vec<Example> {
        D::examples()
    }

    fn run(
        &self,
        engine_state: &EngineState,
        stack: &mut Stack,
        call: &Call,
        input: PipelineData,
    ) -> Result<nu_protocol::PipelineData, nu_protocol::ShellError> {
        let cell_paths: Vec<CellPath> = call.rest(engine_state, stack, 0)?;

        input.map(
            move |v| {
                if cell_paths.is_empty() {
                    action::<D>(&v)
                } else {
                    let mut v = v;
                    for path in &cell_paths {
                        let ret = v
                            .update_cell_path(&path.members, Box::new(move |old| action::<D>(old)));
                        if let Err(error) = ret {
                            return Value::Error { error };
                        }
                    }
                    v
                }
            },
            engine_state.ctrlc.clone(),
        )
    }
}

pub fn action<D>(input: &Value) -> Value
where
    D: HashDigest,
    digest::Output<D>: core::fmt::LowerHex,
{
    let (bytes, span) = match input {
        Value::String { val, span } => (val.as_bytes(), *span),
        Value::Binary { val, span } => (val.as_slice(), *span),
        other => {
            return Value::Error {
                error: ShellError::UnsupportedInput(
                    format!(
                        "Type `{}` is not supported for {} hashing input",
                        other.get_type(),
                        D::name()
                    ),
                    input.span().unwrap_or_else(|_| Span::unknown()),
                ),
            };
        }
    };

    let val = format!("{:x}", D::digest(bytes));
    Value::String { val, span }
}
