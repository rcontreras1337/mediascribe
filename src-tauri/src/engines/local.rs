//! Local transcription engine using `whisper-rs` (whisper.cpp bindings).
//!
//! Implementation is gated behind the `local-engine` feature flag (or
//! `cuda` for GPU acceleration), so the project compiles on machines
//! without LLVM/libclang. Build with:
//!
//! ```bash
//! cargo build --features local-engine     # CPU
//! cargo build --features cuda             # GPU (requires CUDA toolkit)
//! ```

#[cfg(feature = "local-engine")]
mod imp {
    use std::path::Path;
    use std::sync::Arc;

    use thiserror::Error;
    use whisper_rs::{
        FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters,
    };

    use crate::srt::Segment;

    #[derive(Clone)]
    pub struct LocalEngine {
        ctx: Arc<WhisperContext>,
    }

    #[derive(Debug, Error)]
    pub enum LocalEngineError {
        #[error("failed to load whisper model from `{path}`: {source}")]
        ModelLoad {
            path: String,
            #[source]
            source: whisper_rs::WhisperError,
        },

        #[error("transcription failed: {0}")]
        Whisper(#[from] whisper_rs::WhisperError),

        #[error("blocking task panicked: {0}")]
        Join(#[from] tokio::task::JoinError),
    }

    impl LocalEngine {
        pub fn new(model_path: &Path) -> Result<Self, LocalEngineError> {
            let ctx = WhisperContext::new_with_params(
                model_path
                    .to_str()
                    .expect("model path must be valid UTF-8"),
                WhisperContextParameters::default(),
            )
            .map_err(|e| LocalEngineError::ModelLoad {
                path: model_path.display().to_string(),
                source: e,
            })?;
            Ok(Self { ctx: Arc::new(ctx) })
        }

        pub async fn transcribe_pcm(
            &self,
            pcm_f32: Vec<f32>,
            language: String,
            initial_prompt: String,
            beam_size: usize,
        ) -> Result<Vec<Segment>, LocalEngineError> {
            let ctx = Arc::clone(&self.ctx);
            tokio::task::spawn_blocking(move || {
                transcribe_blocking(&ctx, pcm_f32, language, initial_prompt, beam_size)
            })
            .await?
        }
    }

    fn transcribe_blocking(
        ctx: &WhisperContext,
        pcm_f32: Vec<f32>,
        language: String,
        initial_prompt: String,
        beam_size: usize,
    ) -> Result<Vec<Segment>, LocalEngineError> {
        let mut state = ctx.create_state()?;

        let mut params = if beam_size > 1 {
            FullParams::new(SamplingStrategy::BeamSearch {
                beam_size: beam_size as i32,
                patience: 1.0,
            })
        } else {
            FullParams::new(SamplingStrategy::Greedy { best_of: 1 })
        };

        params.set_language(Some(language.as_str()));
        params.set_initial_prompt(initial_prompt.as_str());
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_print_special(false);
        params.set_temperature(0.0);
        params.set_no_speech_thold(0.6);
        params.set_logprob_thold(-1.0);

        state.full(params, &pcm_f32)?;

        let n = state.full_n_segments()?;
        let mut segments = Vec::with_capacity(n as usize);
        for i in 0..n {
            let text = state.full_get_segment_text(i)?;
            let start_cs = state.full_get_segment_t0(i)?;
            let end_cs = state.full_get_segment_t1(i)?;
            segments.push(Segment {
                start: start_cs as f64 / 100.0,
                end: end_cs as f64 / 100.0,
                text,
            });
        }
        Ok(segments)
    }
}

#[cfg(feature = "local-engine")]
pub use imp::*;

#[cfg(not(feature = "local-engine"))]
mod stub {
    //! Compile-only stub when the `local-engine` feature is off. Lets the
    //! rest of the app compile and surfaces a clear error if anyone tries
    //! to use the local engine without it.

    use thiserror::Error;

    #[derive(Debug, Error)]
    pub enum LocalEngineError {
        #[error(
            "local engine is disabled. Rebuild with `--features local-engine` (CPU) \
             or `--features cuda` (GPU) and ensure LLVM is installed for bindgen."
        )]
        Disabled,
    }
}

#[cfg(not(feature = "local-engine"))]
pub use stub::*;
