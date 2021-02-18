use crate::error;
use crate::plots::{Plot, PlotData};
use crate::primitives::{Measurement, TimeInstance};
use crate::util::Result;
use snafu::ensure;

pub struct AreaLineChart {
    timestamps: Vec<TimeInstance>,
    values: Vec<f64>,
    measurement: Measurement,
}

impl AreaLineChart {
    pub fn new(
        timestamps: Vec<TimeInstance>,
        values: Vec<f64>,
        measurement: Measurement,
    ) -> Result<Self> {
        ensure!(
            timestamps.len() == values.len(),
            error::Plot {
                details: "`timestamps` length must equal `values` length"
            }
        );

        Ok(Self {
            timestamps,
            values,
            measurement,
        })
    }
}

impl Plot for AreaLineChart {
    type PlotDataMetadataType = ();

    fn to_vega_embeddable(
        &self,
        _allow_interactions: bool,
    ) -> Result<PlotData<Self::PlotDataMetadataType>> {
        let data = self
            .timestamps
            .iter()
            .zip(&self.values)
            .map(|(timestamp, value)| {
                serde_json::json!({
                    "x": timestamp.as_rfc3339(),
                    "y": value,
                })
            })
            .collect::<Vec<_>>();

        let x_axis_label = "Time";
        let y_axis_label = self.measurement.to_string();

        let vega_string = serde_json::json!({
            "$schema": "https://vega.github.io/schema/vega-lite/v4.17.0.json",
            "data": {
                "values": data
            },
            "description": "Area Plot",
            "encoding": {
                "x": {
                    "field": "x",
                    "title": x_axis_label,
                    "type": "temporal"
                },
                "y": {
                    "field": "y",
                    "title": y_axis_label,
                    "type": "quantitative"
                }
            },
            "mark": {
                "type": "area",
                "line": true,
                "point": true
            }
        })
        .to_string();

        Ok(PlotData {
            vega_string,
            metadata: (),
        })
    }

    fn to_png(&self, _width_px: u16, _height_px: u16) -> Vec<u8> {
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn serialization() {
        let chart = AreaLineChart::new(
            vec![
                TimeInstance::from(NaiveDate::from_ymd(2010, 1, 1).and_hms(0, 0, 0)),
                TimeInstance::from(NaiveDate::from_ymd(2011, 1, 1).and_hms(0, 0, 0)),
                TimeInstance::from(NaiveDate::from_ymd(2012, 1, 1).and_hms(0, 0, 0)),
                TimeInstance::from(NaiveDate::from_ymd(2013, 1, 1).and_hms(0, 0, 0)),
                TimeInstance::from(NaiveDate::from_ymd(2014, 1, 1).and_hms(0, 0, 0)),
            ],
            vec![0., 1., 4., 9., 7.],
            Measurement::Unitless,
        )
        .unwrap();

        assert_eq!(
            chart.to_vega_embeddable(false).unwrap(),
            PlotData {
                vega_string: "".to_string(),
                metadata: (),
            }
        );
    }
}
