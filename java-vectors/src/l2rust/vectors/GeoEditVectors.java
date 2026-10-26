package l2rust.vectors;

import java.io.ByteArrayOutputStream;
import java.lang.reflect.Field;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;

import org.l2jmobius.gameserver.config.GeoEngineConfig;
import org.l2jmobius.gameserver.geoengine.geodata.Cell;
import org.l2jmobius.gameserver.geoengine.geodata.IBlock;
import org.l2jmobius.gameserver.geoengine.geodata.IRegion;
import org.l2jmobius.gameserver.geoengine.geodata.blocks.ComplexBlock;
import org.l2jmobius.gameserver.geoengine.geodata.blocks.FlatBlock;
import org.l2jmobius.gameserver.geoengine.geodata.blocks.MultilayerBlock;
import org.l2jmobius.gameserver.geoengine.geodata.regions.Region;

/**
 * Runtime geodata edits (L-07): {@code Region.setNearestNswe} / {@code unsetNearestNswe} (with {@code convertFlatToComplex}) and
 * {@code Region.saveToFile}, on a synthetic region built here, so the Rust test needs no geodata folder.
 * <p>
 * Same rules as the other generators: no clock, no randomness. The input region and the saved file are written as runs of
 * identical blocks ({@code count} times the block bytes {@code hex}, type byte included), in file order.
 */
public final class GeoEditVectors
{
	/** The region the cells belong to (global geo coordinates), only to exercise the {@code % 256} of {@code getBlock}. */
	private static final int RX = 20;
	private static final int RY = 18;

	private GeoEditVectors()
	{
	}

	private static int word(int height, int nswe)
	{
		return ((height << 1) & 0xFFF0) | nswe;
	}

	private static byte[] flat(int height)
	{
		final ByteBuffer bb = ByteBuffer.allocate(3).order(ByteOrder.LITTLE_ENDIAN);
		bb.put((byte) IBlock.TYPE_FLAT);
		bb.putShort((short) height);
		return bb.array();
	}

	private static byte[] complex()
	{
		final ByteBuffer bb = ByteBuffer.allocate(1 + (IBlock.BLOCK_CELLS * 2)).order(ByteOrder.LITTLE_ENDIAN);
		bb.put((byte) IBlock.TYPE_COMPLEX);
		for (int i = 0; i < IBlock.BLOCK_CELLS; i++)
		{
			bb.putShort((short) word(-40 + (8 * i), i % 16));
		}
		return bb.array();
	}

	/** Cell {@code i} has {@code (i % 4) + 1} layers at -200, 100, 400, 700; cell 9 has two layers at 0 and 160 (a tie at 80). */
	private static byte[] multilayer(int seed)
	{
		final ByteArrayOutputStream out = new ByteArrayOutputStream();
		out.write(IBlock.TYPE_MULTILAYER);
		for (int i = 0; i < IBlock.BLOCK_CELLS; i++)
		{
			final int[] heights = (i == 9) ? new int[]
			{
				0,
				160
			} : Arrays.copyOf(new int[]
			{
				-200,
				100,
				400,
				700
			}, (i % 4) + 1);
			out.write(heights.length);
			for (int k = 0; k < heights.length; k++)
			{
				final int w = word(heights[k] + (seed * 8), (i + k + seed) % 16);
				out.write(w & 0xFF);
				out.write((w >> 8) & 0xFF);
			}
		}
		return out.toByteArray();
	}

	private static byte[] input()
	{
		final ByteArrayOutputStream out = new ByteArrayOutputStream();
		for (int block = 0; block < IRegion.REGION_BLOCKS; block++)
		{
			final byte[] b = switch (block)
			{
				case 258 -> flat(100); // bx 1, by 2: height not a multiple of 8
				case 768 -> flat(-20000); // bx 3, by 0
				case 770 -> flat(20000); // bx 3, by 2
				case 771 -> flat(-7); // bx 3, by 3
				case 1285 -> complex(); // bx 5, by 5
				case 2567 -> multilayer(0); // bx 10, by 7
				case 65535 -> multilayer(1); // last block
				default -> flat(block < 32768 ? 16 : -64);
			};
			out.writeBytes(b);
		}
		return out.toByteArray();
	}

	/** One edit: set or unset, the cell inside the region, the world Z and the bits. */
	private record Edit(String op, int x, int y, int z, int nswe)
	{
	}

	private static final Edit[] EDITS =
	{
		new Edit("unset", 8 + 3, 16 + 4, 0, Cell.NSWE_NORTH), // flat 100 -> complex
		new Edit("set", 8 + 3, 16 + 4, 0, Cell.NSWE_NORTH),
		new Edit("unset", 8 + 3, 16 + 4, 0, Cell.NSWE_EAST | Cell.NSWE_WEST),
		new Edit("set", 8 + 0, 16 + 0, 0, Cell.NSWE_EAST), // already open: no change
		new Edit("set", 0, 0, 0, Cell.NSWE_EAST), // flat: nothing happens
		new Edit("unset", 24 + 0, 0 + 1, -20000, Cell.NSWE_SOUTH), // flat -20000 -> complex (overflow)
		new Edit("unset", 24 + 1, 16 + 0, 20000, Cell.NSWE_WEST), // flat 20000 -> complex (overflow)
		new Edit("unset", 24 + 7, 24 + 7, 0, Cell.NSWE_ALL), // flat -7 -> complex
		new Edit("unset", 24 + 7, 24 + 7, 0, Cell.NSWE_NORTH), // nothing left to remove
		new Edit("set", 40 + 1, 40 + 1, 0, Cell.NSWE_NORTH | Cell.NSWE_EAST), // cell 9: nswe 9 (N|E): no change
		new Edit("set", 40 + 1, 40 + 2, 0, Cell.NSWE_NORTH | Cell.NSWE_EAST), // cell 10: nswe 10 (N|W): N set, so no change
		new Edit("set", 40 + 0, 40 + 4, 0, Cell.NSWE_NORTH | Cell.NSWE_WEST), // cell 4: nswe 4 (S): both added
		new Edit("unset", 40 + 1, 40 + 3, 0, Cell.NSWE_NORTH | Cell.NSWE_EAST), // cell 11: nswe 11 (N|W|E): N and E removed
		new Edit("unset", 40 + 2, 40 + 0, 0, Cell.NSWE_SOUTH), // cell 16: nswe 0: nothing to remove
		new Edit("set", 40 + 7, 40 + 7, 0, Cell.NSWE_ALL), // cell 63: nswe 15
		new Edit("unset", 40 + 7, 40 + 7, 0, Cell.NSWE_SOUTH),
		new Edit("unset", 80 + 0, 56 + 3, 100, Cell.NSWE_ALL), // cell 3 (4 layers): exact layer 100
		new Edit("unset", 80 + 0, 56 + 3, 260, Cell.NSWE_ALL), // between 100 and 400: 400 is nearer (140 against 160)
		new Edit("set", 80 + 0, 56 + 3, 100, Cell.NSWE_NORTH),
		new Edit("unset", 80 + 1, 56 + 1, 80, Cell.NSWE_ALL), // cell 9: tie between 0 and 160, the first layer wins
		new Edit("unset", 80 + 1, 56 + 2, 30000, Cell.NSWE_EAST), // cell 10 (3 layers): far above
		new Edit("set", 80 + 1, 56 + 2, -30000, Cell.NSWE_ALL), // far below
		new Edit("unset", 2047, 2047, 0, Cell.NSWE_WEST), // last block, last cell
	};

	/**
	 * Writes {@code geo_edit.json}.
	 * @param outDir the output folder
	 * @throws Exception if the Java classes fail
	 */
	public static void generate(Path outDir) throws Exception
	{
		final byte[] input = input();
		final Region region = new Region(ByteBuffer.wrap(input).order(ByteOrder.LITTLE_ENDIAN));

		// An untouched region saves to the very bytes it was loaded from.
		final Path dir = Files.createTempDirectory("l2rust-geo-edit");
		GeoEngineConfig.GEOEDIT_PATH = dir;
		check(region.saveToFile("untouched.l2j"), "save untouched");
		check(Arrays.equals(Files.readAllBytes(dir.resolve("untouched.l2j")), input), "untouched save differs from the input");

		final List<Object> edits = new ArrayList<>();
		for (Edit e : EDITS)
		{
			final int geoX = (RX * IRegion.REGION_CELLS_X) + e.x;
			final int geoY = (RY * IRegion.REGION_CELLS_Y) + e.y;
			if (e.op.equals("set"))
			{
				region.setNearestNswe(geoX, geoY, e.z, (byte) e.nswe);
			}
			else
			{
				region.unsetNearestNswe(geoX, geoY, e.z, (byte) e.nswe);
			}
			final Map<String, Object> o = Json.obj();
			o.put("op", e.op);
			o.put("geo_x", geoX);
			o.put("geo_y", geoY);
			o.put("world_z", e.z);
			o.put("nswe", e.nswe);
			o.put("block_type_after", typeOf(blockAt(region, geoX, geoY)));
			o.put("nearest_z_after", region.getNearestZ(geoX, geoY, e.z));
			int open = 0;
			for (int dir2 : new int[]
			{
				Cell.NSWE_EAST,
				Cell.NSWE_WEST,
				Cell.NSWE_SOUTH,
				Cell.NSWE_NORTH
			})
			{
				if (region.checkNearestNswe(geoX, geoY, e.z, dir2))
				{
					open |= dir2;
				}
			}
			o.put("nswe_after", open);
			edits.add(Json.compact(o));
		}

		check(region.saveToFile("edited.l2j"), "save edited");
		final byte[] saved = Files.readAllBytes(dir.resolve("edited.l2j"));
		Files.delete(dir.resolve("untouched.l2j"));
		Files.delete(dir.resolve("edited.l2j"));
		Files.delete(dir);

		final Map<String, Object> root = Json.header("l2jmobius Region.setNearestNswe / unsetNearestNswe (ComplexBlock, MultilayerBlock, convertFlatToComplex) and Region.saveToFile");
		root.put("note", "A synthetic region (blocks in file order, runs of identical blocks: count x hex, type byte included) is loaded with new Region(LITTLE_ENDIAN), saved untouched (equal to the input), then edited in order and saved again. geo_x/geo_y are global (region " + RX + "_" + RY + "). nswe_after = the open bits of checkNearestNswe at world_z after the edit; block_type_after: 0 flat, 1 complex, 2 multilayer.");
		root.put("region", RX + "_" + RY);
		root.put("input_len", input.length);
		root.put("input", runs(input));
		root.put("edits", edits);
		root.put("saved_len", saved.length);
		root.put("saved", runs(saved));
		Json.write(outDir.resolve("geo_edit.json"), root);
	}

	/** Splits a region file into blocks and merges equal neighbours into runs. */
	private static List<Object> runs(byte[] file)
	{
		final List<Object> out = new ArrayList<>();
		int pos = 0;
		byte[] current = null;
		int count = 0;
		while (pos < file.length)
		{
			final int start = pos;
			final int type = file[pos++];
			if (type == IBlock.TYPE_FLAT)
			{
				pos += 2;
			}
			else if (type == IBlock.TYPE_COMPLEX)
			{
				pos += IBlock.BLOCK_CELLS * 2;
			}
			else
			{
				for (int i = 0; i < IBlock.BLOCK_CELLS; i++)
				{
					final int layers = file[pos++];
					pos += layers * 2;
				}
			}
			final byte[] block = Arrays.copyOfRange(file, start, pos);
			if ((current != null) && Arrays.equals(current, block))
			{
				count++;
				continue;
			}
			if (current != null)
			{
				out.add(run(count, current));
			}
			current = block;
			count = 1;
		}
		if (current != null)
		{
			out.add(run(count, current));
		}
		return out;
	}

	private static Object run(int count, byte[] block)
	{
		final Map<String, Object> o = Json.obj();
		o.put("count", count);
		o.put("hex", Json.hex(block));
		return Json.compact(o);
	}

	private static IBlock blockAt(Region region, int geoX, int geoY) throws Exception
	{
		final Field f = Region.class.getDeclaredField("_blocks");
		f.setAccessible(true);
		final IBlock[] blocks = (IBlock[]) f.get(region);
		return blocks[(((geoX / IBlock.BLOCK_CELLS_X) % IRegion.REGION_BLOCKS_X) * IRegion.REGION_BLOCKS_Y) + ((geoY / IBlock.BLOCK_CELLS_Y) % IRegion.REGION_BLOCKS_Y)];
	}

	private static int typeOf(IBlock block)
	{
		if (block instanceof FlatBlock)
		{
			return IBlock.TYPE_FLAT;
		}
		if (block instanceof ComplexBlock)
		{
			return IBlock.TYPE_COMPLEX;
		}
		if (block instanceof MultilayerBlock)
		{
			return IBlock.TYPE_MULTILAYER;
		}
		throw new IllegalStateException("unknown block class: " + block.getClass());
	}

	private static void check(boolean ok, String message)
	{
		if (!ok)
		{
			throw new IllegalStateException("vector self-check failed: " + message);
		}
	}
}
