import { createMemo, createSignal, Show } from "solid-js";
import { avatarHue, avatarSource, initials } from "../lib/format";

export default function Avatar(props: { name: string; email: string; size?: number; color?: string; picture?: string | null }) {
  const size = () => props.size ?? 40;
  const hue = () => avatarHue(props.email || props.name);
  const photo = createMemo(() => avatarSource(props.picture));
  const [failed, setFailed] = createSignal<string | null>(null);
  return (
    <div
      class="grid shrink-0 place-items-center overflow-hidden rounded-full font-medium text-white"
      style={{
        width: `${size()}px`,
        height: `${size()}px`,
        "font-size": `${Math.round(size() * 0.4)}px`,
        background: props.color || `hsl(${hue()} 45% 45%)`,
      }}
      aria-hidden="true"
    >
      <Show when={photo() !== failed() ? photo() : null} fallback={initials(props.name, props.email)}>
        {(source) => <img src={source()} alt="" class="size-full object-cover" decoding="async" referrerpolicy="no-referrer" onError={() => setFailed(source())} />}
      </Show>
    </div>
  );
}
