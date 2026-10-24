import java.awt.Color;
import java.awt.Font;
import java.awt.Graphics2D;
import java.awt.image.BufferedImage;
import java.io.File;
import javax.imageio.ImageIO;

/** 生成模拟器相机的固定合成图；不读取外部图片、字体或用户文件。 */
final class OcrCameraImage {
    public static void main(String[] arguments) throws Exception {
        BufferedImage image = new BufferedImage(1024, 1024, BufferedImage.TYPE_INT_RGB);
        Graphics2D canvas = image.createGraphics();
        try {
            canvas.setColor(Color.WHITE);
            canvas.fillRect(0, 0, image.getWidth(), image.getHeight());
            canvas.setColor(Color.BLACK);
            canvas.setRenderingHint(java.awt.RenderingHints.KEY_TEXT_ANTIALIASING,
                java.awt.RenderingHints.VALUE_TEXT_ANTIALIAS_ON);
            canvas.setFont(new Font(Font.SANS_SERIF, Font.PLAIN, 40));
            String text = "SYNTHETIC OCR 2468";
            var metrics = canvas.getFontMetrics();
            // 两版 AVD 的纵向 imagefile 照片已确认能看到左半幅，完整文本放入该范围，保留原有精确断言。
            if (metrics.stringWidth(text) > image.getWidth() / 2) throw new IllegalStateException("fixture_width");
            canvas.drawString(text, (image.getWidth() / 2 - metrics.stringWidth(text)) / 2,
                (image.getHeight() - metrics.getHeight()) / 2 + metrics.getAscent());
        } finally { canvas.dispose(); }
        if (!ImageIO.write(image, "png", new File(arguments[0]))) throw new IllegalStateException("fixture_image");
    }
}
