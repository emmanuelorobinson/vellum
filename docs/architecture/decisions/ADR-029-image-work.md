# ADR-029 — REST import and local/generated image work

Status: implemented boundaries; native writable-editor attachment remains subject to ADR-022.
Date: 2026-10-21

Figma import uses public REST JSON with `geometry=paths`, then image-fill URLs from the file-images
endpoint. A personal token comes only from SecretStore (`figma_token`) and travels only in the
X-Figma-Token header to api.figma.com. Every API/CDN origin is separately authorized by Outside before
fetching. The adapter limits CDN hosts to Figma/S3, strips credentials from downloads, and never follows
redirects. It never reads proprietary .fig data. Geometry is flattened to editable absolute layers;
unsupported content produces explicit notes, with no promises of source fidelity.

ImageGenerator and ImageOps return bytes/specs, with fakes and shared contracts. The image generator
uses the documented inline GPT Image protocol at an explicitly configured HTTPS endpoint. The model
is person-configured, not hardcoded. Only base64 output is accepted: no arbitrary result URL fetching.
Errors never preserve provider response payloads. Generated fills store provider/model in metadata,
without prompts, credentials or provider response bodies. Import/fill_image previews omit raw inputs.

ImageOps uses image 0.25.10 for bounded decode/PNG encoding, stable vtracer 0.6.5 for tracing, and stable
tract-onnx 0.23.9 (transformers disabled) for self-contained CPU inference. No runtime/model downloads
occur in product code. Explicit local models use the U2Net-style NCHW 320 x 320 input contract and one
foreground output, with ImageNet normalization. Model provisioning/licensing and segmentation quality
are the person's responsibility; the synthetic ONNX lane proves inference plumbing only. A model
incompatible with tract fails explicitly. The prerelease ort experiment was removed to preserve the
no-prerelease lockfile rule. vtracer's stable release brings deprecated adler/ansi_term/atty dependencies;
required advisory checks remain failures, with no policy exception.

Pixels stay in AssetStore. Remove-background updates the original image reference; tracing replaces a
leaf image/one-image-fill shape with an editable frame of vectors, retaining its id, metadata, fixed size
and placement. Tracing intentionally reports loss of crop/fit/decorations. Each operation commits one
Origin-tagged transaction and can be undone independently. No format change, DocStore side door or
vendor format becomes a system of record. Provider keys live only in the OS keychain.

Non-secret native settings live in image-settings.json alongside daemon state; CLI configuration and
stdin-to-keychain commands are available without an account. The C25 provider/model/key controls are
host ports; the currently read-only desktop exposes Figma import as unavailable rather than faking a
person-origin import. Completing the native authoring callbacks depends on Prompt 55's writable ABI.
