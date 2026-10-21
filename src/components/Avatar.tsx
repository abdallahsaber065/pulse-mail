import { avatarHue, initials } from "../lib/format";

export default function Avatar(props: { name: string; email: string; size?: number; color?: string }) {
  const size = () => props.size ?? 40;
  const hue = () => avatarHue(props.email || props.name);
  return (
    <div
      class="grid shrink-0 place-items-center rounded-full font-medium text-white"
      style={{
        width: `${size()}px`,
        height: `${size()}px`,
        "font-size": `${Math.round(size() * 0.4)}px`,
        background: props.color || `hsl(${hue()} 45% 45%)`,
      }}
      aria-hidden="true"
    >
      {initials(props.name, props.email)}
    </div>
  );
}
