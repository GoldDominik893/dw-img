import tkinter as tk
from tkinter import filedialog, messagebox
from PIL import Image, ImageTk
import zlib
import os

# Function to view a DWXG image file using the old image viewer with zoom and scroll
def view_dwxg(file_path):
    def zoom(event):
        nonlocal zoom_level, img, width, height
        zoom_factor = 1.1 if event.delta > 0 else 1 / 1.1
        new_zoom_level = zoom_level * zoom_factor

        if 0.1 < new_zoom_level < 10:
            mouse_x, mouse_y = canvas.canvasx(event.x), canvas.canvasy(event.y)
            zoom_level = new_zoom_level
            new_width, new_height = int(width * zoom_level), int(height * zoom_level)
            resized_img = img.resize((new_width, new_height), Image.NEAREST)
            img_tk = ImageTk.PhotoImage(resized_img)
            canvas.itemconfig(image_id, image=img_tk)
            canvas.image = img_tk
            canvas.config(scrollregion=(0, 0, new_width, new_height))

    try:
        with open(file_path, 'rb') as f:
            magic = f.read(4)
            if magic != b'DWXG':
                raise ValueError("Invalid DWXG file")
            width = int.from_bytes(f.read(4), 'big')
            height = int.from_bytes(f.read(4), 'big')
            is_compressed = bool(int.from_bytes(f.read(4), 'big'))
            color_depth = int.from_bytes(f.read(4), 'big')
            data_length = int.from_bytes(f.read(4), 'big')
            pixels = f.read(data_length)

            if is_compressed:
                pixels = zlib.decompress(pixels)

        # Reconstruct the image based on color depth
        if color_depth == 16:
            img = Image.new('RGB', (width, height))
            pixels_data = img.load()
            idx = 0
            for y in range(height):
                for x in range(width):
                    rgb565 = int.from_bytes(pixels[idx:idx+2], 'big')
                    r5 = (rgb565 >> 11) & 0x1F
                    g6 = (rgb565 >> 5) & 0x3F
                    b5 = rgb565 & 0x1F
                    r = r5 << 3
                    g = g6 << 2
                    b = b5 << 3
                    pixels_data[x, y] = (r, g, b)
                    idx += 2
        else:
            messagebox.showerror("Error", "Unsupported color depth.")
            return

        # Create the viewer window
        zoom_level = 1.0
        viewer_window = tk.Toplevel()
        viewer_window.title(f"Viewing: {file_path}")
        viewer_window.geometry("800x600")
        viewer_window.resizable(True, True)

        canvas = tk.Canvas(viewer_window, bg="white")
        h_scroll = tk.Scrollbar(viewer_window, orient=tk.HORIZONTAL, command=canvas.xview)
        v_scroll = tk.Scrollbar(viewer_window, orient=tk.VERTICAL, command=canvas.yview)
        canvas.configure(xscrollcommand=h_scroll.set, yscrollcommand=v_scroll.set)

        h_scroll.pack(side=tk.BOTTOM, fill=tk.X)
        v_scroll.pack(side=tk.RIGHT, fill=tk.Y)
        canvas.pack(side=tk.LEFT, fill=tk.BOTH, expand=True)

        img_tk = ImageTk.PhotoImage(img)
        image_id = canvas.create_image(0, 0, anchor="nw", image=img_tk)
        canvas.image = img_tk
        canvas.config(scrollregion=canvas.bbox(tk.ALL))
        canvas.bind("<MouseWheel>", zoom)

        # Show file metadata
        metadata_label = tk.Label(viewer_window, text=f"Resolution: {width}x{height}\nCompressed: {is_compressed}\nFile Size: {os.path.getsize(file_path)} bytes")
        metadata_label.pack(side=tk.BOTTOM, pady=10)

    except Exception as e:
        messagebox.showerror("Error", f"Failed to view DWXG file: {e}")

# Function to convert an image to DWXG format with optional compression and other settings
def convert_to_dwxg(image_path, output_path, enable_compression=False, enable_dithering=False, color_depth=16, rotate=0, flip=None, resize=None):
    try:
        # Open the image and convert to RGB
        img = Image.open(image_path).convert('RGB')  # Convert to RGB mode
        if rotate:
            img = img.rotate(rotate, expand=True)  # Rotate the image
        if flip == 'horizontal':
            img = img.transpose(Image.FLIP_LEFT_RIGHT)  # Flip image horizontally
        elif flip == 'vertical':
            img = img.transpose(Image.FLIP_TOP_BOTTOM)  # Flip image vertically
        if resize:
            img = img.resize(resize)  # Resize the image

        width, height = img.size
        if enable_dithering:
            img = img.convert('RGB', dither=Image.FLOYDSTEINBERG)
        pixels = img.load()

        # Prepare pixel data based on selected color depth
        pixel_data = bytearray()
        for y in range(height):
            for x in range(width):
                r, g, b = pixels[x, y]
                if color_depth == 16:  # RGB565
                    r5 = (r >> 3) & 0x1F
                    g6 = (g >> 2) & 0x3F
                    b5 = (b >> 3) & 0x1F
                    rgb565 = (r5 << 11) | (g6 << 5) | b5
                    pixel_data.extend(rgb565.to_bytes(2, 'big'))
                elif color_depth == 8:  # Grayscale
                    gray = int(0.299 * r + 0.587 * g + 0.114 * b)  # Convert to grayscale
                    pixel_data.append(gray)  # Use 1 byte per pixel

        # Optional compression
        if enable_compression:
            pixel_data = zlib.compress(pixel_data)

        # Create DWXG file
        with open(output_path, 'wb') as f:
            # Write header
            f.write(b'DWXG')  # Magic identifier
            f.write(width.to_bytes(4, 'big'))
            f.write(height.to_bytes(4, 'big'))
            f.write((1 if enable_compression else 0).to_bytes(4, 'big'))  # Compression flag
            f.write(color_depth.to_bytes(4, 'big'))  # Color depth
            f.write(len(pixel_data).to_bytes(4, 'big'))  # Data length

            # Write pixel data
            f.write(pixel_data)

        messagebox.showinfo("Success", f"Image converted to DWXG format and saved to {output_path}")
    except Exception as e:
        messagebox.showerror("Error", f"Failed to convert image: {e}")

# Function to open the conversion window and allow user to select options
def open_conversion_window():
    conversion_window = tk.Toplevel()
    conversion_window.title("Convert Image to DWXG")
    conversion_window.geometry("450x400")
    conversion_window.resizable(False, False)

    # Create widgets for image selection
    image_label = tk.Label(conversion_window, text="Select Image:")
    image_label.grid(row=0, column=0, sticky="e", padx=5, pady=5)

    image_entry = tk.Entry(conversion_window, width=30)
    image_entry.grid(row=0, column=1, padx=5, pady=5)

    def browse_image():
        file_path = filedialog.askopenfilename(title="Select Image", filetypes=[("Image Files", "*.png;*.jpg;*.jpeg;*.bmp")])
        if file_path:
            image_entry.delete(0, tk.END)
            image_entry.insert(0, file_path)

    browse_button = tk.Button(conversion_window, text="...", command=browse_image)
    browse_button.grid(row=0, column=2, padx=5, pady=5)

    # Output file selection
    output_label = tk.Label(conversion_window, text="Output File:")
    output_label.grid(row=1, column=0, sticky="e", padx=5, pady=5)

    output_entry = tk.Entry(conversion_window, width=30)
    output_entry.grid(row=1, column=1, padx=5, pady=5)

    def browse_output():
        file_path = filedialog.asksaveasfilename(defaultextension=".dwxg", filetypes=[("DWXG Files", "*.dwxg")])
        if file_path:
            output_entry.delete(0, tk.END)
            output_entry.insert(0, file_path)

    browse_output_button = tk.Button(conversion_window, text="...", command=browse_output)
    browse_output_button.grid(row=1, column=2, padx=5, pady=5)

    # Compression option
    compression_var = tk.BooleanVar()
    compression_check = tk.Checkbutton(conversion_window, text="Enable Compression", variable=compression_var)
    compression_check.grid(row=2, column=0, columnspan=3, pady=5)

    # Dithering option
    dithering_var = tk.BooleanVar()
    dithering_check = tk.Checkbutton(conversion_window, text="Enable Dithering", variable=dithering_var)
    dithering_check.grid(row=3, column=0, columnspan=3, pady=5)

    # Color depth selection
    color_depth_label = tk.Label(conversion_window, text="Select Color Depth:")
    color_depth_label.grid(row=4, column=0, sticky="e", padx=5, pady=5)

    color_depth_var = tk.StringVar()
    color_depth_var.set("16")  # Default is 16-bit
    color_depth_menu = tk.OptionMenu(conversion_window, color_depth_var, "16", "8")
    color_depth_menu.grid(row=4, column=1, padx=5, pady=5)

    # Image Rotation option
    rotation_label = tk.Label(conversion_window, text="Rotate Image:")
    rotation_label.grid(row=5, column=0, sticky="e", padx=5, pady=5)

    rotation_var = tk.IntVar()
    rotation_var.set(0)  # Default is no rotation
    rotation_menu = tk.OptionMenu(conversion_window, rotation_var, 0, 90, 180, 270)
    rotation_menu.grid(row=5, column=1, padx=5, pady=5)

    # Image flip option
    flip_label = tk.Label(conversion_window, text="Flip Image:")
    flip_label.grid(row=6, column=0, sticky="e", padx=5, pady=5)

    flip_var = tk.StringVar()
    flip_var.set("None")
    flip_menu = tk.OptionMenu(conversion_window, flip_var, "None", "horizontal", "vertical")
    flip_menu.grid(row=6, column=1, padx=5, pady=5)

    # Resize image option
    resize_label = tk.Label(conversion_window, text="Resize Image (WxH):")
    resize_label.grid(row=7, column=0, sticky="e", padx=5, pady=5)

    resize_width_entry = tk.Entry(conversion_window, width=10)
    resize_width_entry.grid(row=7, column=1, padx=5, pady=5)

    resize_height_entry = tk.Entry(conversion_window, width=10)
    resize_height_entry.grid(row=7, column=2, padx=5, pady=5)

    # Function to create DWXG file
    def create_dwxg():
        image_path = image_entry.get()
        output_path = output_entry.get()
        enable_compression = compression_var.get()
        enable_dithering = dithering_var.get()
        color_depth = int(color_depth_var.get())
        rotate = rotation_var.get()
        flip = flip_var.get() if flip_var.get() != "None" else None
        resize = None
        if resize_width_entry.get() and resize_height_entry.get():
            resize = (int(resize_width_entry.get()), int(resize_height_entry.get()))

        if not image_path or not output_path:
            messagebox.showerror("Error", "Please provide both image and output file paths.")
            return

        convert_to_dwxg(image_path, output_path, enable_compression, enable_dithering, color_depth, rotate, flip, resize)
        conversion_window.destroy()  # Close the conversion window after conversion

    create_button = tk.Button(conversion_window, text="Create DWXG", command=create_dwxg)
    create_button.grid(row=8, column=0, columnspan=3, pady=10)

# Main GUI
def main():
    root = tk.Tk()
    root.title("DWXG Image Converter")
    root.geometry("400x300")
    root.resizable(False, False)

    title_label = tk.Label(root, text="DWXG Image Converter", font=("Arial", 16, "bold"))
    title_label.pack(pady=10)

    instruction_label = tk.Label(root, text="Convert and view 16-bit color images in the custom DWXG format.", wraplength=350)
    instruction_label.pack(pady=5)

    convert_button = tk.Button(root, text="Convert Image to DWXG", command=open_conversion_window, bg="#4CAF50", fg="white", font=("Arial", 12))
    convert_button.pack(pady=10)

    # View button for viewing DWXG files
    def open_viewer():
        file_path = filedialog.askopenfilename(title="Select DWXG File", filetypes=[("DWXG Files", "*.dwxg")])
        if file_path:
            view_dwxg(file_path)

    view_button = tk.Button(root, text="View DWXG File", command=open_viewer, bg="#2196F3", fg="white", font=("Arial", 12))
    view_button.pack(pady=10)

    root.mainloop()

if __name__ == "__main__":
    main()
