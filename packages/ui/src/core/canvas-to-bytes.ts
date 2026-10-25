interface CanvasEncodingOptions {
  type: string;
  quality?: number;
  createError: () => Error;
}

/** 把画布编码为字节；格式、质量和空结果的错误由调用方决定。 */
export function canvasToBytes(
  canvas: HTMLCanvasElement,
  { type, quality, createError }: CanvasEncodingOptions,
): Promise<Uint8Array> {
  return new Promise((resolve, reject) =>
    canvas.toBlob(
      (blob) => {
        if (!blob) reject(createError());
        else void blob.arrayBuffer().then((buffer) => resolve(new Uint8Array(buffer)), reject);
      },
      type,
      quality,
    ),
  );
}
