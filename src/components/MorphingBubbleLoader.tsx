import { createSignal, onMount, onCleanup } from "solid-js";

export default function MorphingBubbleLoader() {
  const [isVisible, setIsVisible] = createSignal(false);

  onMount(() => {
    setIsVisible(true);
  });

  return (
    <div class="relative w-12 h-12 flex items-center justify-center">
      <div class="absolute inset-0 flex items-center justify-center">
        {/* Bubble 1 */}
        <div
          class="absolute w-2 h-2 bg-dark rounded-full opacity-80"
          style={{
            animation: "float1 3s ease-in-out infinite",
          }}
        />

        {/* Bubble 2 */}
        <div
          class="absolute w-2 h-2 bg-dark rounded-full opacity-70"
          style={{
            animation: "float2 3s ease-in-out infinite -0.5s",
          }}
        />

        {/* Bubble 3 */}
        <div
          class="absolute w-2 h-2 bg-dark rounded-full opacity-75"
          style={{
            animation: "float3 3s ease-in-out infinite -1s",
          }}
        />

        {/* Bubble 4 */}
        <div
          class="absolute w-2 h-2 bg-dark rounded-full opacity-65"
          style={{
            animation: "float4 3s ease-in-out infinite -1.5s",
          }}
        />
      </div>

      <style>
        {`
          @keyframes float1 {
            0%, 100% { transform: translate(-6px, -6px) scale(1); }
            50% { transform: translate(6px, 6px) scale(1.1); }
          }
          
          @keyframes float2 {
            0%, 100% { transform: translate(6px, -6px) scale(1); }
            50% { transform: translate(-6px, 6px) scale(1.1); }
          }
          
          @keyframes float3 {
            0%, 100% { transform: translate(-6px, 6px) scale(1); }
            50% { transform: translate(6px, -6px) scale(1.1); }
          }
          
          @keyframes float4 {
            0%, 100% { transform: translate(6px, 6px) scale(1); }
            50% { transform: translate(-6px, -6px) scale(1.1); }
          }
        `}
      </style>
    </div>
  );
}
