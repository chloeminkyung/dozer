use crate::pipeline::builder::SchemaSQLContext;
use crate::pipeline::{projection::factory::ProjectionProcessorFactory, tests::utils::get_select};
use dozer_core::channels::ProcessorChannelForwarder;
use dozer_core::node::ProcessorFactory;
use dozer_core::DEFAULT_PORT_HANDLE;
use dozer_types::types::{Field, Operation, Record, Schema};
use std::collections::HashMap;

struct TestChannelForwarder {
    operations: Vec<Operation>,
}

impl ProcessorChannelForwarder for TestChannelForwarder {
    fn send(
        &mut self,
        op: dozer_types::types::Operation,
        _port: dozer_core::node::PortHandle,
    ) -> Result<(), dozer_core::errors::ExecutionError> {
        self.operations.push(op);
        Ok(())
    }
}

pub(crate) fn run_scalar_fct(sql: &str, schema: Schema, input: Vec<Field>) -> Field {
    let select = get_select(sql).unwrap();
    let processor_factory = ProjectionProcessorFactory::_new(select.projection);
    processor_factory
        .get_output_schema(
            &DEFAULT_PORT_HANDLE,
            &[(
                DEFAULT_PORT_HANDLE,
                (schema.clone(), SchemaSQLContext::default()),
            )]
            .into_iter()
            .collect(),
        )
        .unwrap();

    let mut processor = processor_factory
        .build(
            HashMap::from([(DEFAULT_PORT_HANDLE, schema)]),
            HashMap::new(),
        )
        .unwrap();

    let mut fw = TestChannelForwarder { operations: vec![] };

    let op = Operation::Insert {
        new: Record::new(None, input, None),
    };

    processor.process(DEFAULT_PORT_HANDLE, op, &mut fw).unwrap();

    match &fw.operations[0] {
        Operation::Insert { new } => new.values[0].clone(),
        _ => panic!("Unable to find result value"),
    }
}
