use crate::datasets::listing::{DatasetListOptions, DatasetListing, DatasetProvider};
use crate::datasets::provenance::{ProvenanceOutput, ProvenanceProvider};
use crate::datasets::storage::DatasetProviderDefinition;
use crate::error::{self, Result};
use crate::projects::{RasterSymbology, Symbology};
use crate::stac::{Feature as StacFeature, FeatureCollection as StacCollection, StacAsset};
use crate::util::user_input::Validated;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use geoengine_datatypes::dataset::{DatasetId, DatasetProviderId, ExternalDatasetId};
use geoengine_datatypes::operations::image::{Colorizer, RgbaColor};
use geoengine_datatypes::operations::reproject::{
    CoordinateProjection, CoordinateProjector, ReprojectClipped,
};
use geoengine_datatypes::primitives::{
    AxisAlignedRectangle, BoundingBox2D, Measurement, SpatialPartitioned, TimeInterval,
};
use geoengine_datatypes::raster::{GeoTransform, RasterDataType};
use geoengine_datatypes::spatial_reference::{SpatialReference, SpatialReferenceAuthority};
use geoengine_operators::engine::{
    MetaData, MetaDataProvider, RasterQueryRectangle, RasterResultDescriptor, VectorQueryRectangle,
    VectorResultDescriptor,
};
use geoengine_operators::mock::MockDatasetDataSourceLoadingInfo;
use geoengine_operators::source::{
    GdalDatasetParameters, GdalLoadingInfo, GdalLoadingInfoPart, GdalLoadingInfoPartIterator,
    OgrSourceDataset,
};
use log::debug;
use serde::{Deserialize, Serialize};
use snafu::ResultExt;
use std::collections::HashMap;
use std::convert::TryInto;
use std::fmt::Debug;
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SentinelS2L2ACogsProviderDefinition {
    name: String,
    id: DatasetProviderId,
    api_url: String,
}

#[typetag::serde]
#[async_trait]
impl DatasetProviderDefinition for SentinelS2L2ACogsProviderDefinition {
    async fn initialize(
        self: Box<Self>,
    ) -> crate::error::Result<Box<dyn crate::datasets::listing::DatasetProvider>> {
        Ok(Box::new(SentinelS2L2aCogsDataProvider::new(
            self.id,
            self.api_url,
        )))
    }

    fn type_name(&self) -> String {
        "SentinelS2L2ACogs".to_owned()
    }

    fn name(&self) -> String {
        self.name.clone()
    }

    fn id(&self) -> DatasetProviderId {
        self.id
    }
}

#[derive(Debug, Clone)]
pub struct Band {
    pub name: String,
    pub no_data_value: Option<f64>,
    pub data_type: RasterDataType,
}

impl Band {
    pub fn new(name: String, no_data_value: Option<f64>, data_type: RasterDataType) -> Self {
        Self {
            name,
            no_data_value,
            data_type,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Zone {
    pub name: String,
    pub epsg: u32,
}

impl Zone {
    pub fn new(name: String, epsg: u32) -> Self {
        Self { name, epsg }
    }
}

#[derive(Debug, Clone)]
pub struct SentinelMetaData {
    bands: Vec<Band>,
    zones: Vec<Zone>,
}

#[derive(Debug, Clone)]
pub struct SentinelDataset {
    band: Band,
    zone: Zone,
    listing: DatasetListing,
}

pub struct SentinelS2L2aCogsDataProvider {
    api_url: String,

    datasets: HashMap<DatasetId, SentinelDataset>,
}

impl SentinelS2L2aCogsDataProvider {
    pub fn new(id: DatasetProviderId, api_url: String) -> Self {
        let meta_data = Self::load_metadata();
        Self {
            api_url,
            datasets: Self::create_datasets(&id, &meta_data),
        }
    }

    fn load_metadata() -> SentinelMetaData {
        // TODO: fetch dataset metadata from config or remote
        SentinelMetaData {
            bands: vec![
                Band::new("B01".to_owned(), Some(0.), RasterDataType::U16),
                Band::new("B02".to_owned(), Some(0.), RasterDataType::U16),
                Band::new("B03".to_owned(), Some(0.), RasterDataType::U16),
                Band::new("B04".to_owned(), Some(0.), RasterDataType::U16),
                Band::new("B08".to_owned(), Some(0.), RasterDataType::U16),
                Band::new("SCL".to_owned(), Some(0.), RasterDataType::U8),
            ],
            zones: vec![
                Zone::new("UTM32N".to_owned(), 32632),
                Zone::new("UTM36S".to_owned(), 32736),
            ],
        }
    }

    fn create_datasets(
        id: &DatasetProviderId,
        meta_data: &SentinelMetaData,
    ) -> HashMap<DatasetId, SentinelDataset> {
        meta_data
            .zones
            .iter()
            .flat_map(|zone| {
                meta_data.bands.iter().map(move |band| {
                    let dataset_id: DatasetId = ExternalDatasetId {
                        provider_id: *id,
                        dataset_id: format!("{}:{}", zone.name, band.name),
                    }
                    .into();
                    let listing = DatasetListing {
                        id: dataset_id.clone(),
                        name: format!("Sentinel S2 L2A COGS {}:{}", zone.name, band.name),
                        description: "".to_owned(),
                        tags: vec![],
                        source_operator: "GdalSource".to_owned(),
                        result_descriptor: RasterResultDescriptor {
                            data_type: band.data_type,
                            spatial_reference: SpatialReference::new(
                                SpatialReferenceAuthority::Epsg,
                                zone.epsg,
                            )
                            .into(),
                            measurement: Measurement::Unitless, // TODO: add measurement
                            no_data_value: band.no_data_value,
                        }
                        .into(),
                        symbology: Some(Symbology::Raster(RasterSymbology {
                            opacity: 1.0,
                            colorizer: Colorizer::linear_gradient(
                                vec![
                                    (0.0, RgbaColor::white())
                                        .try_into()
                                        .expect("valid breakpoint"),
                                    (10_000.0, RgbaColor::black())
                                        .try_into()
                                        .expect("valid breakpoint"),
                                ],
                                RgbaColor::transparent(),
                                RgbaColor::transparent(),
                            )
                            .expect("valid colorizer"),
                        })), // TODO: individual colorizer per band
                    };

                    let dataset = SentinelDataset {
                        zone: zone.clone(),
                        band: band.clone(),
                        listing,
                    };

                    (dataset_id, dataset)
                })
            })
            .collect()
    }
}

#[async_trait]
impl DatasetProvider for SentinelS2L2aCogsDataProvider {
    async fn list(&self, _options: Validated<DatasetListOptions>) -> Result<Vec<DatasetListing>> {
        // TODO: options
        let mut x: Vec<DatasetListing> =
            self.datasets.values().map(|d| d.listing.clone()).collect();
        x.sort_by_key(|e| e.name.clone());
        Ok(x)
    }

    async fn load(
        &self,
        _dataset: &geoengine_datatypes::dataset::DatasetId,
    ) -> crate::error::Result<crate::datasets::storage::Dataset> {
        Err(error::Error::NotYetImplemented)
    }
}

#[async_trait]
impl ProvenanceProvider for SentinelS2L2aCogsDataProvider {
    async fn provenance(&self, dataset: &DatasetId) -> Result<ProvenanceOutput> {
        Ok(ProvenanceOutput {
            dataset: dataset.clone(),
            provenance: None, // TODO
        })
    }
}

#[derive(Debug, Clone)]
pub struct SentinelS2L2aCogsMetaData {
    api_url: String,
    zone: Zone,
    band: Band,
}

impl SentinelS2L2aCogsMetaData {
    async fn create_loading_info(&self, query: RasterQueryRectangle) -> Result<GdalLoadingInfo> {
        // for reference: https://stacspec.org/STAC-ext-api.html#operation/getSearchSTAC
        debug!("create_loading_info with: {:?}", &query);
        let request_params = self.request_params(query)?;
        debug!("queried with: {:?}", &request_params);
        let features = self.load_all_features(&request_params).await?;
        debug!("number of features returned by STAC: {}", features.len());
        let mut features: Vec<StacFeature> = features
            .into_iter()
            .filter(|f| {
                f.properties
                    .proj_epsg
                    .map_or(false, |epsg| epsg == self.zone.epsg)
            })
            .collect();

        features.sort_by_key(|a| a.properties.datetime);

        let mut parts = vec![];
        let num_features = features.len();
        debug!("number of features in current zone: {}", num_features);
        for i in 0..num_features {
            let feature = &features[i];

            let start = feature.properties.datetime;
            // feature is valid until next feature starts
            let end = if i < num_features - 1 {
                features[i + 1].properties.datetime
            } else {
                start + Duration::seconds(1) // TODO: determine correct validity for last tile
            };

            let time_interval = TimeInterval::new(start, end)?;

            if time_interval.intersects(&query.time_interval) {
                debug!(
                    "STAC asset time: {}, url: {}",
                    time_interval,
                    feature
                        .assets
                        .get(&self.band.name)
                        .map_or(&"n/a".to_string(), |a| &a.href)
                );

                let asset =
                    feature
                        .assets
                        .get(&self.band.name)
                        .ok_or(error::Error::StacNoSuchBand {
                            band_name: self.band.name.clone(),
                        })?;

                parts.push(self.create_loading_info_part(time_interval, asset)?);
            }
        }
        debug!("number of generated loading infos: {}", parts.len());

        Ok(GdalLoadingInfo {
            info: GdalLoadingInfoPartIterator::Static {
                parts: parts.into_iter(),
            },
        })
    }

    fn create_loading_info_part(
        &self,
        time_interval: TimeInterval,
        asset: &StacAsset,
    ) -> Result<GdalLoadingInfoPart> {
        let [stac_shape_y, stac_shape_x] = asset.proj_shape.ok_or(error::Error::StacInvalidBbox)?;

        Ok(GdalLoadingInfoPart {
            time: time_interval,
            params: GdalDatasetParameters {
                file_path: PathBuf::from(format!("/vsicurl/{}", asset.href)),
                rasterband_channel: 1,
                geo_transform: GeoTransform::from(
                    asset
                        .gdal_geotransform()
                        .ok_or(error::Error::StacInvalidGeoTransform)?,
                ),
                width: stac_shape_x as usize,
                height: stac_shape_y as usize,
                file_not_found_handling: geoengine_operators::source::FileNotFoundHandling::NoData,
                no_data_value: self.band.no_data_value,
                properties_mapping: None,
                gdal_open_options: None,
            },
        })
    }

    fn request_params(&self, query: RasterQueryRectangle) -> Result<Vec<(String, String)>> {
        let (t_start, t_end) = Self::time_range_request(&query.time_interval)?;

        // request all features in zone in order to be able to determine the temporal validity of individual tile
        let projector = CoordinateProjector::from_known_srs(
            SpatialReference::new(SpatialReferenceAuthority::Epsg, self.zone.epsg),
            SpatialReference::epsg_4326(),
        )?;

        let spatial_partition = query.spatial_partition(); // TODO: use SpatialPartition2D directly
        let bbox = BoundingBox2D::new_upper_left_lower_right_unchecked(
            spatial_partition.upper_left(),
            spatial_partition.lower_right(),
        );
        let bbox = bbox.reproject_clipped(&projector)?; // TODO: use reproject_clipped on SpatialPartition2D

        Ok(vec![
            (
                "collections[]".to_owned(),
                "sentinel-s2-l2a-cogs".to_owned(),
            ),
            (
                "bbox".to_owned(),
                format!(
                    "[{},{},{},{}]", // array-brackets are not used in standard but required here for unknkown reason
                    bbox.lower_left().x,
                    bbox.lower_left().y,
                    bbox.upper_right().x,
                    bbox.upper_right().y
                ),
            ), // TODO: order coordinates depending on projection
            (
                "datetime".to_owned(),
                format!("{}/{}", t_start.to_rfc3339(), t_end.to_rfc3339()),
            ),
            ("limit".to_owned(), "500".to_owned()),
        ])
    }

    async fn load_all_features<T: Serialize + ?Sized + Debug>(
        &self,
        params: &T,
    ) -> Result<Vec<StacFeature>> {
        let mut features = vec![];

        let mut collection = self.load_collection(params, 1).await?;
        features.append(&mut collection.features);

        let num_pages =
            (collection.context.matched as f64 / collection.context.limit as f64).ceil() as u32;

        for page in 2..=num_pages {
            let mut collection = self.load_collection(params, page).await?;
            features.append(&mut collection.features);
        }

        Ok(features)
    }

    async fn load_collection<T: Serialize + ?Sized + Debug>(
        &self,
        params: &T,
        page: u32,
    ) -> Result<StacCollection> {
        let client = reqwest::Client::new();
        let text = client
            .get(&self.api_url)
            .query(&params)
            .query(&[("page", &page.to_string())])
            .send()
            .await
            .context(error::Reqwest)?
            .text()
            .await
            .context(error::Reqwest)?;

        serde_json::from_str(&text).map_err(|error| error::Error::StacJsonResponse {
            url: self.api_url.clone(),
            response: text,
            error,
        })
    }

    fn time_range_request(time: &TimeInterval) -> Result<(DateTime<Utc>, DateTime<Utc>)> {
        let t_start =
            time.start()
                .as_utc_date_time()
                .ok_or(geoengine_operators::error::Error::DataType {
                    source: geoengine_datatypes::error::Error::NoDateTimeValid {
                        time_instance: time.start(),
                    },
                })?;

        // shift start by 1 minute to ensure getting the most recent data for start time
        let t_start = t_start - Duration::minutes(1);

        let t_end =
            time.end()
                .as_utc_date_time()
                .ok_or(geoengine_operators::error::Error::DataType {
                    source: geoengine_datatypes::error::Error::NoDateTimeValid {
                        time_instance: time.end(),
                    },
                })?;

        Ok((t_start, t_end))
    }
}

#[async_trait]
impl MetaData<GdalLoadingInfo, RasterResultDescriptor, RasterQueryRectangle>
    for SentinelS2L2aCogsMetaData
{
    async fn loading_info(
        &self,
        query: RasterQueryRectangle,
    ) -> geoengine_operators::util::Result<GdalLoadingInfo> {
        // TODO: propagate error properly
        debug!("loading_info for: {:?}", &query);
        self.create_loading_info(query).await.map_err(|e| {
            geoengine_operators::error::Error::LoadingInfo {
                source: Box::new(e),
            }
        })
    }

    async fn result_descriptor(&self) -> geoengine_operators::util::Result<RasterResultDescriptor> {
        Ok(RasterResultDescriptor {
            data_type: self.band.data_type,
            spatial_reference: SpatialReference::new(
                SpatialReferenceAuthority::Epsg,
                self.zone.epsg,
            )
            .into(),
            measurement: Measurement::Unitless,
            no_data_value: self.band.no_data_value,
        })
    }

    fn box_clone(
        &self,
    ) -> Box<dyn MetaData<GdalLoadingInfo, RasterResultDescriptor, RasterQueryRectangle>> {
        Box::new(self.clone())
    }
}

#[async_trait]
impl MetaDataProvider<GdalLoadingInfo, RasterResultDescriptor, RasterQueryRectangle>
    for SentinelS2L2aCogsDataProvider
{
    async fn meta_data(
        &self,
        dataset: &DatasetId,
    ) -> Result<
        Box<dyn MetaData<GdalLoadingInfo, RasterResultDescriptor, RasterQueryRectangle>>,
        geoengine_operators::error::Error,
    > {
        let dataset = self
            .datasets
            .get(dataset)
            .ok_or(geoengine_operators::error::Error::UnknownDatasetId)?;

        Ok(Box::new(SentinelS2L2aCogsMetaData {
            api_url: self.api_url.clone(),
            zone: dataset.zone.clone(),
            band: dataset.band.clone(),
        }))
    }
}

#[async_trait]
impl
    MetaDataProvider<MockDatasetDataSourceLoadingInfo, VectorResultDescriptor, VectorQueryRectangle>
    for SentinelS2L2aCogsDataProvider
{
    async fn meta_data(
        &self,
        _dataset: &DatasetId,
    ) -> Result<
        Box<
            dyn MetaData<
                MockDatasetDataSourceLoadingInfo,
                VectorResultDescriptor,
                VectorQueryRectangle,
            >,
        >,
        geoengine_operators::error::Error,
    > {
        Err(geoengine_operators::error::Error::NotImplemented)
    }
}

#[async_trait]
impl MetaDataProvider<OgrSourceDataset, VectorResultDescriptor, VectorQueryRectangle>
    for SentinelS2L2aCogsDataProvider
{
    async fn meta_data(
        &self,
        _dataset: &DatasetId,
    ) -> Result<
        Box<dyn MetaData<OgrSourceDataset, VectorResultDescriptor, VectorQueryRectangle>>,
        geoengine_operators::error::Error,
    > {
        Err(geoengine_operators::error::Error::NotImplemented)
    }
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::BufReader, str::FromStr};

    use futures::StreamExt;
    use geoengine_datatypes::primitives::{SpatialPartition2D, SpatialResolution};
    use geoengine_operators::{
        engine::{MockExecutionContext, MockQueryContext, RasterOperator},
        source::{FileNotFoundHandling, GdalSource, GdalSourceParameters},
    };

    use super::*;

    #[tokio::test]
    async fn loading_info() -> Result<()> {
        // TODO: mock STAC endpoint

        let def: Box<dyn DatasetProviderDefinition> = serde_json::from_reader(BufReader::new(
            File::open("services/test-data/provider_defs/pro/sentinel_s2_l2a_cogs.json")?,
        ))?;

        let provider = def.initialize().await?;

        let meta: Box<dyn MetaData<GdalLoadingInfo, RasterResultDescriptor, RasterQueryRectangle>> =
            provider
                .meta_data(
                    &ExternalDatasetId {
                        provider_id: DatasetProviderId::from_str(
                            "5779494c-f3a2-48b3-8a2d-5fbba8c5b6c5",
                        )?,
                        dataset_id: "UTM32N:B01".to_owned(),
                    }
                    .into(),
                )
                .await
                .unwrap();

        let loading_info = meta
            .loading_info(RasterQueryRectangle {
                spatial_bounds: SpatialPartition2D::new_unchecked(
                    (166_021.44, 0.00).into(),
                    (534_994.66, 9_329_005.18).into(),
                ),
                time_interval: TimeInterval::new_instant(
                    DateTime::parse_from_rfc3339("2021-01-02T10:02:26Z")
                        .unwrap()
                        .timestamp_millis(),
                )?,
                spatial_resolution: SpatialResolution::one(),
            })
            .await
            .unwrap();

        let expected = vec![GdalLoadingInfoPart {
            time: TimeInterval::new_unchecked(1_609_581_746_000, 1_609_581_747_000),
            params: GdalDatasetParameters {
                file_path: "/vsicurl/https://sentinel-cogs.s3.us-west-2.amazonaws.com/sentinel-s2-l2a-cogs/32/R/PU/2021/1/S2B_32RPU_20210102_0_L2A/B01.tif".into(),
                rasterband_channel: 1,
                geo_transform: GeoTransform {
                    origin_coordinate: (600_000.0, 3_400_020.0).into(),
                    x_pixel_size: 60.,
                    y_pixel_size: -60.,
                },
                width: 1830,
                height: 1830,
                file_not_found_handling: FileNotFoundHandling::NoData,
                no_data_value: Some(0.),
                properties_mapping: None,
                gdal_open_options: None,
            },
        }];

        if let GdalLoadingInfoPartIterator::Static { parts } = loading_info.info {
            let result: Vec<_> = parts.collect();

            assert_eq!(result.len(), 1);

            assert_eq!(result, expected);
        } else {
            unreachable!();
        }

        Ok(())
    }

    #[tokio::test]
    async fn query_data() -> Result<()> {
        // TODO: mock STAC endpoint

        let mut exe = MockExecutionContext::default();

        let def: Box<dyn DatasetProviderDefinition> = serde_json::from_reader(BufReader::new(
            File::open("services/test-data/provider_defs/pro/sentinel_s2_l2a_cogs.json")?,
        ))?;

        let provider = def.initialize().await?;

        let meta: Box<dyn MetaData<GdalLoadingInfo, RasterResultDescriptor, RasterQueryRectangle>> =
            provider
                .meta_data(
                    &ExternalDatasetId {
                        provider_id: DatasetProviderId::from_str(
                            "5779494c-f3a2-48b3-8a2d-5fbba8c5b6c5",
                        )?,
                        dataset_id: "UTM32N:B01".to_owned(),
                    }
                    .into(),
                )
                .await?;

        exe.add_meta_data(
            ExternalDatasetId {
                provider_id: DatasetProviderId::from_str("5779494c-f3a2-48b3-8a2d-5fbba8c5b6c5")?,
                dataset_id: "UTM32N:B01".to_owned(),
            }
            .into(),
            meta,
        );

        let op = GdalSource {
            params: GdalSourceParameters {
                dataset: ExternalDatasetId {
                    provider_id: DatasetProviderId::from_str(
                        "5779494c-f3a2-48b3-8a2d-5fbba8c5b6c5",
                    )?,
                    dataset_id: "UTM32N:B01".to_owned(),
                }
                .into(),
            },
        }
        .boxed()
        .initialize(&exe)
        .await
        .unwrap();

        let processor = op.query_processor()?.get_u16().unwrap();

        let query = RasterQueryRectangle {
            spatial_bounds: SpatialPartition2D::new_unchecked(
                (166_021.44, 9_329_005.18).into(),
                (534_994.66, 0.00).into(),
            ),
            time_interval: TimeInterval::new_instant(
                DateTime::parse_from_rfc3339("2021-01-02T10:02:26Z")
                    .unwrap()
                    .timestamp_millis(),
            )?,
            spatial_resolution: SpatialResolution::new_unchecked(
                166_021.44 / 256.,
                (9_329_005.18 - 534_994.66) / 256.,
            ),
        };

        let ctx = MockQueryContext::new(usize::MAX);

        let result = processor
            .raster_query(query, &ctx)
            .await?
            .collect::<Vec<_>>()
            .await;

        // TODO: check actual data
        assert_eq!(result.len(), 2);

        Ok(())
    }
}
