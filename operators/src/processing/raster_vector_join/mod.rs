use crate::engine::{
    ExecutionContext, InitializedOperator, InitializedOperatorImpl, InitializedVectorOperator,
    Operator, TypedVectorQueryProcessor, VectorOperator, VectorResultDescriptor,
};
use crate::error;
use crate::util::Result;

use geoengine_datatypes::collections::VectorDataType;
use serde::{Deserialize, Serialize};
use snafu::ensure;

/// An operator that attaches raster values to vector data
pub type RasterVectorJoin = Operator<RasterVectorJoinParams>;

/// The parameter spec for `RasterVectorJoin`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RasterVectorJoinParams {
    /// Each name reflects the output column of the join result.
    /// For each raster input, one name must be defined.
    pub names: Vec<String>,
}

const MAX_NUMBER_OF_RASTER_INPUTS: usize = 8;

#[typetag::serde]
impl VectorOperator for RasterVectorJoin {
    fn initialize(
        mut self: Box<Self>,
        context: &ExecutionContext<'_>,
    ) -> Result<Box<InitializedVectorOperator>> {
        ensure!(
            self.vector_sources.len() == 1,
            error::InvalidNumberOfVectorInputs {
                expected: 1..2,
                found: self.vector_sources.len()
            }
        );

        ensure!(
            !self.raster_sources.is_empty()
                || self.raster_sources.len() > MAX_NUMBER_OF_RASTER_INPUTS,
            error::InvalidNumberOfRasterInputs {
                expected: 1..MAX_NUMBER_OF_RASTER_INPUTS,
                found: self.raster_sources.len()
            }
        );
        ensure!(
            self.raster_sources.len() == self.params.names.len(),
            error::InvalidOperatorSpec {
                reason: "`raster_sources` must be of equal length as `names`"
            }
        );

        let vector_source = self.vector_sources.remove(0).initialize(context)?;

        ensure!(
            vector_source.result_descriptor().data_type != VectorDataType::Data,
            error::InvalidType {
                expected: format!(
                    "{}, {} or {}",
                    VectorDataType::MultiPoint,
                    VectorDataType::MultiLineString,
                    VectorDataType::MultiPolygon
                ),
                found: VectorDataType::Data.to_string()
            },
        );

        // TODO: check for column clashes earlier with the result descriptor
        // TODO: update result descriptor with new column(s)

        Ok(InitializedRasterVectorJoin {
            params: self.params,
            raster_sources: self
                .raster_sources
                .into_iter()
                .map(|source| source.initialize(context))
                .collect::<Result<Vec<_>>>()?,
            result_descriptor: vector_source.result_descriptor(),
            vector_sources: vec![vector_source],
            state: (),
        }
        .boxed())
    }
}

pub type InitializedRasterVectorJoin =
    InitializedOperatorImpl<RasterVectorJoinParams, VectorResultDescriptor, ()>;

impl InitializedOperator<VectorResultDescriptor, TypedVectorQueryProcessor>
    for InitializedRasterVectorJoin
{
    fn query_processor(&self) -> Result<TypedVectorQueryProcessor> {
        todo!("implement")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use serde_json::json;

    #[test]
    fn serialization() {
        let raster_vector_join = RasterVectorJoin {
            params: RasterVectorJoinParams {
                names: ["foo", "bar"].iter().cloned().map(str::to_string).collect(),
            },
            raster_sources: vec![],
            vector_sources: vec![],
        };

        let serialized = json!({
            "type": "RasterVectorJoin",
            "params": {
                "names": ["foo", "bar"],
            },
            "raster_sources": [],
            "vector_sources": [],
        })
        .to_string();

        let deserialized: RasterVectorJoin = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.params, raster_vector_join.params);
    }
}
