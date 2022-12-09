| offset | size | name | description |
|-|-|-|-|
| 0 | 4 | toc_offset | file offset to table of content (toc) |
| 4 | toc_offset - 4 | data | lz4 compressed file data |
| toc_offset | 4 | entry_count | number of file entries |
||| first file entry ||
| toc_offset +  4 |  4 | file_type | 0 - image (even though filenames have ".png", it's actually a bc7 texture with mipmaps), 1 - sound |
| toc_offset +  8 |  4 | size_decompressed | decompressed size |
| toc_offset + 12 |  4 | size | size of compressed data in bigblob |
| toc_offset + 16 | 24 | unks | unknown, sound files always have 0s, for images maybe xy coords for something? |
| toc_offset + 16 |  8 | original_canvas_size? | |
| toc_offset + 24 |  8 | original_canvas_offset? | |
| toc_offset + 32 |  8 | original_canvas_crop_size? | |
| toc_offset + 40 |  4 | width | width of an image, 0 for sound files |
| toc_offset + 44 |  4 | height | height of an image, 0 for sound files |
| toc_offset + 48 |  4 | offset | offset in bigblob |
| toc_offset + 52 |  4 | name_len | length of file name in bytes |
| toc_offset + 56 | name_len | name | file name |
||| next entries | same as first, repeated until end of file |