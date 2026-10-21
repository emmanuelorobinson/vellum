import type { AuthoringController } from "@vellum/editor-core";
import { useStore } from "../hooks";
import { Icon } from "../icons/Icon";
export function ImageActions({
  authoring: a,
  address,
  disabled = false,
}: {
  authoring: AuthoringController;
  address: string;
  disabled?: boolean;
}) {
  useStore(a.subscribe);
  const blocked = disabled || a.state.busy || !a.host.processImage;
  return (
    <section className="vx-image-actions" aria-label="Selected image">
      <h3>Selected image</h3>
      <button
        type="button"
        className="vx-button"
        data-primary=""
        disabled={blocked}
        onClick={() => void a.processImage(address, "remove_background")}
      >
        Remove background
      </button>
      <button
        type="button"
        className="vx-button"
        disabled={blocked}
        onClick={() => void a.processImage(address, "trace")}
      >
        Convert to vectors
      </button>
      <p>Best for logos and flat artwork.</p>
      {!a.host.processImage ? <p>Local image processing is unavailable in this host.</p> : null}
      {a.state.busy ? (
        <p role="status">
          <Icon name="loader-circle" size={16} />
          Processing image…
        </p>
      ) : null}
    </section>
  );
}
