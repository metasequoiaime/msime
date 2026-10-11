/** 触发浏览器下载，并在浏览器接手后释放临时链接。 */
export function downloadBlob(blob: Blob, filename: string): void {
  const anchor = document.createElement("a");
  const url = URL.createObjectURL(blob);
  try {
    anchor.href = url;
    anchor.download = filename;
    document.body.appendChild(anchor);
    anchor.click();
  } finally {
    anchor.remove();
    // 保留云词库使用的延迟，避免浏览器读取前撤销 Blob URL。
    window.setTimeout(() => URL.revokeObjectURL(url), 1000);
  }
}
