import { getCurrentWindow } from "@tauri-apps/api/window";

export default function TitleBar() {
  const appWindow = getCurrentWindow();

  const handleMouseDown = async (e: MouseEvent) => {
    await appWindow.startDragging();
  };

  return (
    <div
      class="fixed top-0 left-0 right-0 h-10 bg-dark flex items-center z-50"
      onMouseDown={handleMouseDown}
      data-tauri-drag-region
    ></div>
  );
}
