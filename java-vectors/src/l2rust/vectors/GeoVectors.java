package l2rust.vectors;

import java.io.RandomAccessFile;
import java.lang.reflect.Field;
import java.nio.ByteOrder;
import java.nio.channels.FileChannel.MapMode;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Random;

import org.l2jmobius.gameserver.geoengine.geodata.Cell;
import org.l2jmobius.gameserver.geoengine.geodata.IBlock;
import org.l2jmobius.gameserver.geoengine.geodata.IRegion;
import org.l2jmobius.gameserver.geoengine.geodata.blocks.ComplexBlock;
import org.l2jmobius.gameserver.geoengine.geodata.blocks.FlatBlock;
import org.l2jmobius.gameserver.geoengine.geodata.blocks.MultilayerBlock;
import org.l2jmobius.gameserver.geoengine.geodata.regions.Region;

/**
 * Geodata vectors for M1 (M1-JV-03). Every value comes from the real L2JMobius {@link Region} class.
 * <p>
 * Rules for generators (same as {@link CryptoVectors}):
 * <ul>
 * <li>No clock, no {@code SecureRandom}. Samples come from {@code new Random(seed)} with a fixed seed.</li>
 * <li>The region is loaded exactly like {@code GeoEngine.loadRegion}: a read-only memory map, little-endian.</li>
 * <li>Geo coordinates are the global ones {@code GeoEngine} uses: region {@code rx_ry} covers
 * {@code geoX} in {@code [rx * 2048, rx * 2048 + 2047]} and {@code geoY} in {@code [ry * 2048, ry * 2048 + 2047]}.</li>
 * </ul>
 * Geodata folders are arguments (see {@link Main}), never hard-coded here. Nothing is read from them except the region files.
 */
public final class GeoVectors
{
	/** Regions written by this generator, as {@code {regionX, regionY}} (file name {@code regionX_regionY.l2j}). */
	private static final int[][] REGIONS =
	{
		{22, 22},
		{14, 23},
		{16, 10},
		{12, 24},
		{13, 21}
	};

	/** Fixed world Z values sampled for every cell, plus the cell's own nearest Z (the sixth choice). */
	private static final int[] FIXED_Z = {-16000, -2000, 0, 2000, 16000};

	private static final int SAMPLES = 2000;
	private static final long SEED = 20260401L;

	private GeoVectors()
	{
	}

	/**
	 * Writes {@code geo_download_<region>.json} from {@code downloadDir} and {@code geo_converted_<region>.json} from {@code convertedDir}.
	 * @param outDir the output folder
	 * @param downloadDir folder with the downloaded {@code .l2j} files
	 * @param convertedDir folder with the converted {@code .l2j} files
	 * @throws Exception if a region file can't be read or the Java classes fail
	 */
	public static void generate(Path outDir, Path downloadDir, Path convertedDir) throws Exception
	{
		for (int[] r : REGIONS)
		{
			dumpRegion(outDir, "download", downloadDir, r[0], r[1]);
		}
		for (int[] r : REGIONS)
		{
			dumpRegion(outDir, "converted", convertedDir, r[0], r[1]);
		}
	}

	private static void dumpRegion(Path outDir, String set, Path geoDir, int regionX, int regionY) throws Exception
	{
		final String fileName = regionX + "_" + regionY + ".l2j";
		final Region region = loadRegion(geoDir.resolve(fileName));
		final IBlock[] blocks = blocksOf(region);

		final int[] counts = new int[3];
		for (IBlock block : blocks)
		{
			counts[typeOf(block)]++;
		}
		check(blocks.length == IRegion.REGION_BLOCKS, "block count");
		check((counts[0] + counts[1] + counts[2]) == IRegion.REGION_BLOCKS, "block type counts do not add up");

		final Random random = new Random(SEED + (regionX * 100L) + regionY);
		final List<Object> samples = new ArrayList<>();
		int fixedCount = 0;
		int ownCount = 0;
		for (int i = 0; i < SAMPLES; i++)
		{
			final int geoX = (regionX * IRegion.REGION_CELLS_X) + random.nextInt(IRegion.REGION_CELLS_X);
			final int geoY = (regionY * IRegion.REGION_CELLS_Y) + random.nextInt(IRegion.REGION_CELLS_Y);
			final int ownZ = region.getNearestZ(geoX, geoY, 0);
			final int pick = random.nextInt(FIXED_Z.length + 1);
			final boolean own = pick == FIXED_Z.length;
			final int worldZ = own ? ownZ : FIXED_Z[pick];
			if (own)
			{
				ownCount++;
			}
			else
			{
				fixedCount++;
			}
			samples.add(sample(region, blocks, geoX, geoY, worldZ, own ? "own_nearest" : "fixed"));
		}
		check((fixedCount + ownCount) == SAMPLES, "sample count");

		final Map<String, Object> root = Json.header("l2jmobius GeoEngine.loadRegion (new Region(LITTLE_ENDIAN)) + Region.getNearestZ/getNextLowerZ/getNextHigherZ/checkNearestNswe; block type via IBlock classes (FlatBlock, ComplexBlock, MultilayerBlock)");
		root.put("note", "geo_x and geo_y are global geo cell coordinates. worldZ is one of -16000, -2000, 0, 2000, 16000 (z_source fixed) or the cell's own nearest Z at worldZ 0 (z_source own_nearest). block_type: 0 flat, 1 complex, 2 multilayer. nswe.* are checkNearestNswe for single directions. seed = 20260401 + regionX * 100 + regionY; the same samples are used for both sets.");
		root.put("set", set);
		root.put("region", regionX + "_" + regionY);
		root.put("file", fileName);
		root.put("seed", SEED + (regionX * 100L) + regionY);
		root.put("samples_count", SAMPLES);
		final Map<String, Object> typeCounts = Json.obj();
		typeCounts.put("flat", counts[IBlock.TYPE_FLAT]);
		typeCounts.put("complex", counts[IBlock.TYPE_COMPLEX]);
		typeCounts.put("multilayer", counts[IBlock.TYPE_MULTILAYER]);
		root.put("block_counts", typeCounts);
		root.put("samples", samples);

		Json.write(outDir.resolve("geo_" + set + "_" + regionX + "_" + regionY + ".json"), root);
	}

	private static Map<String, Object> sample(Region region, IBlock[] blocks, int geoX, int geoY, int worldZ, String zSource)
	{
		final Map<String, Object> o = Json.obj();
		o.put("geo_x", geoX);
		o.put("geo_y", geoY);
		o.put("world_z", worldZ);
		o.put("z_source", zSource);
		o.put("block_type", typeOf(blockAt(blocks, geoX, geoY)));
		o.put("nearest_z", region.getNearestZ(geoX, geoY, worldZ));
		o.put("next_lower_z", region.getNextLowerZ(geoX, geoY, worldZ));
		o.put("next_higher_z", region.getNextHigherZ(geoX, geoY, worldZ));

		final Map<String, Object> nswe = Json.obj();
		nswe.put("east", region.checkNearestNswe(geoX, geoY, worldZ, Cell.NSWE_EAST));
		nswe.put("west", region.checkNearestNswe(geoX, geoY, worldZ, Cell.NSWE_WEST));
		nswe.put("south", region.checkNearestNswe(geoX, geoY, worldZ, Cell.NSWE_SOUTH));
		nswe.put("north", region.checkNearestNswe(geoX, geoY, worldZ, Cell.NSWE_NORTH));
		o.put("nswe", nswe);
		return o;
	}

	/** Same block index formula as {@code Region.getBlock}, which is private. */
	private static IBlock blockAt(IBlock[] blocks, int geoX, int geoY)
	{
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

	/** The private {@code _blocks} array of a loaded region (the blocks are not reachable any other way). */
	private static IBlock[] blocksOf(Region region) throws Exception
	{
		final Field f = Region.class.getDeclaredField("_blocks");
		f.setAccessible(true);
		return (IBlock[]) f.get(region);
	}

	/** Loads a region like {@code GeoEngine.loadRegion}. */
	private static Region loadRegion(Path file) throws Exception
	{
		try (RandomAccessFile raf = new RandomAccessFile(file.toFile(), "r"))
		{
			return new Region(raf.getChannel().map(MapMode.READ_ONLY, 0, raf.length()).order(ByteOrder.LITTLE_ENDIAN));
		}
	}

	private static void check(boolean ok, String message)
	{
		if (!ok)
		{
			throw new IllegalStateException("vector self-check failed: " + message);
		}
	}
}
