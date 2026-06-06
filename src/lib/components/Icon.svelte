<script lang="ts">
  import { icons } from "lucide";

  let { name, class: cls = "ic" }: { name: string; class?: string } = $props();

  // kebab-case data-lucide name → PascalCase key in the lucide `icons` map
  const pascal = name
    .split("-")
    .map((s) => s.charAt(0).toUpperCase() + s.slice(1))
    .join("");

  const node = (icons as Record<string, [string, Record<string, string | number>][]>)[pascal];

  // Сериализуем дочерние элементы иконки (path/circle/line...) в строку для {@html}.
  const inner = node
    ? node
        .map(
          ([tag, attrs]) =>
            `<${tag} ${Object.entries(attrs)
              .map(([k, v]) => `${k}="${v}"`)
              .join(" ")} />`,
        )
        .join("")
    : "";
</script>

<svg
  class={cls}
  xmlns="http://www.w3.org/2000/svg"
  width="24"
  height="24"
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
>{@html inner}</svg>
