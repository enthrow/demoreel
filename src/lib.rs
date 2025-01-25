#[macro_use]
extern crate error_chain;

pub mod errors;
pub mod serialize;
pub mod tracer;

use bitbuffer::BitRead;
use pyo3::prelude::*;
use pyo3_polars::PyDataFrame;
use pythonize::pythonize;
use serde_arrow::schema::TracingOptions;
use tf_demo_parser::demo::{header::Header, parser::DemoParser};
use tf_demo_parser::Demo;
use tracer::{Roster, Tracer, WithTick};

use errors::*;
use serialize::to_polars;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracer::PacketStream;
    const BORNEO: &'static [u8] = include_bytes!("../demos/Round_1_Map_1_Borneo.dem");
    const FLAG_UPDATES: &'static [u8] = include_bytes!("../demos/flag_updates.dem");
    #[test]
    fn dtrace_succeeds() {
        Python::with_gil(|py| {
            assert!(dtrace(py, BORNEO).is_ok());
            // assert!(roster(py, PAYLOAD).is_ok());
        });
    }

    #[test]
    fn log_flag_updates() {
        let demo = Demo::new(FLAG_UPDATES);
        let packets = PacketStream::new(demo).unwrap();

        for result in packets {
            let packet = result.unwrap();
            match &packet {
                tf_demo_parser::demo::packet::Packet::ConsoleCmd(cmd) => {
                    if cmd.command.starts_with("echo ") {
                        println!("{}", cmd.command);
                    }
                }
                _ => {}
            }
        }
    }
}

fn is_pov_formatted(s: &str) -> bool {
    if let Some((hostname, port)) = s.split_once(':') {
        !hostname.is_empty() && port.parse::<u16>().is_ok()
    } else {
        false
    }
}

#[pyclass(get_all)]
pub struct DTrace {
    header: Option<PyObject>,
    states: Option<PyDataFrame>,
    events: Option<PyDataFrame>,
    roster: Option<PyDataFrame>,
    bounds: Option<PyDataFrame>,
}

#[pyfunction]
fn roster<'py>(py: Python<'py>, buffer: &[u8]) -> Result<Option<PyDataFrame>> {
    py.allow_threads(|| -> Result<_> {
        let demo = Demo::new(&buffer);
        let stream = demo.get_stream();
        let parser = DemoParser::new_with_analyser(stream, Roster::new());
        let (_header, roster) = parser.parse()?;
        Ok(to_polars(roster.roster.as_slice(), None)?.map(PyDataFrame))
    })
}

/// see if the server in the header is formatted like a hostname to determine if this is a pov demo
/// This isn't foolproof and could be combined with checking header.nick against roster and for
/// the presence of user commands if we want to account for intentional misrepresentation of this by players.
/// returns a bool.
#[pyfunction]
fn is_pov<'py>(py: Python<'py>, buffer: &[u8]) -> Result<Option<bool>> {
    py.allow_threads(|| -> Result<_> {
        let demo = Demo::new(&buffer);
        let mut stream = demo.get_stream();
        let header = Header::read(&mut stream)?;
        Ok(Some(is_pov_formatted(&header.server)))
    })
}

#[pyfunction]
fn header<'py>(py: Python<'py>, buffer: &[u8]) -> Result<Option<PyObject>> {
    let header = py.allow_threads(|| -> Result<_> {
        let demo = Demo::new(&buffer);
        let mut stream = demo.get_stream();
        let header = Header::read(&mut stream)?;
        Ok(Some(header))
    })?;
    Ok(Some(pythonize(py, &header).unwrap().into()))
}

/// Trace all players, states, and instances of damage inflicted within a
/// demo file, yielding the result as a set of polars dataframes.
#[pyfunction]
#[pyo3(signature = (buffer))]
fn dtrace<'py>(py: Python<'py>, buffer: &[u8]) -> Result<DTrace> {
    let (header, states, events, roster, bounds) = py.allow_threads(|| -> Result<_> {
        let demo = Demo::new(&buffer);
        let stream = demo.get_stream();
        let parser = DemoParser::new_with_analyser(stream, Tracer::new());
        let (header, dtrace) = parser.parse()?;
        let tropt = TracingOptions::default()
            .allow_null_fields(true)
            .string_dictionary_encoding(false);
        let states = WithTick::to_polars(dtrace.states.into_iter(), Some(tropt.clone()))?;
        let events = WithTick::to_polars(dtrace.events.into_iter(), Some(tropt.clone()))?;
        let bounds = WithTick::to_polars(dtrace.bounds.into_iter(), Some(tropt.clone()))?;
        let roster = to_polars(dtrace.roster.roster.as_slice(), Some(tropt.clone()))?;
        Ok((
            header,
            states.map(PyDataFrame),
            events.map(PyDataFrame),
            roster.map(PyDataFrame),
            bounds.map(PyDataFrame),
        ))
    })?;
    let header = Some(pythonize(py, &header).unwrap().into());

    let dtrace = DTrace {
        header,
        states,
        events,
        roster,
        bounds,
    };
    Ok(dtrace)
}

#[pymodule]
fn demoreel(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(dtrace, m)?)?;
    m.add_function(wrap_pyfunction!(roster, m)?)?;
    m.add_function(wrap_pyfunction!(header, m)?)?;
    m.add_function(wrap_pyfunction!(is_pov, m)?)?;
    Ok(())
}
