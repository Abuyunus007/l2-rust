package l2rust.vectors;

import java.io.File;
import java.lang.reflect.Constructor;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.logging.Filter;
import java.util.logging.Handler;
import java.util.logging.Level;
import java.util.logging.LogRecord;
import java.util.logging.Logger;

import org.l2jmobius.commons.threads.ThreadPool;
import org.l2jmobius.gameserver.config.ConfigLoader;
import org.l2jmobius.gameserver.config.ServerConfig;
import org.l2jmobius.gameserver.data.xml.ArmorSetData;
import org.l2jmobius.gameserver.data.xml.EnchantItemHPBonusData;
import org.l2jmobius.gameserver.data.xml.ExperienceData;
import org.l2jmobius.gameserver.data.xml.HennaData;
import org.l2jmobius.gameserver.data.xml.HitConditionBonusData;
import org.l2jmobius.gameserver.data.xml.ItemData;
import org.l2jmobius.gameserver.data.xml.NpcData;
import org.l2jmobius.gameserver.data.xml.PlayerTemplateData;
import org.l2jmobius.gameserver.data.xml.SkillData;
import org.l2jmobius.gameserver.data.xml.SkillTreeData;
import org.l2jmobius.gameserver.entity.actor.Npc;
import org.l2jmobius.gameserver.entity.actor.Player;
import org.l2jmobius.gameserver.entity.actor.appearance.PlayerAppearance;
import org.l2jmobius.gameserver.mechanics.skill.holders.SkillLearn;
import org.l2jmobius.gameserver.entity.actor.templates.NpcTemplate;
import org.l2jmobius.gameserver.entity.actor.templates.PlayerTemplate;
import org.l2jmobius.gameserver.entity.item.Weapon;
import org.l2jmobius.gameserver.entity.item.instance.Item;
import org.l2jmobius.gameserver.entity.itemcontainer.Inventory;
import org.l2jmobius.gameserver.mechanics.skill.Skill;

/**
 * Minimal in-process start of the real L2JMobius game server code for the stat and formula vector generators (M4-JV-01). No database, no network.
 * <p>
 * What is started, in this order (all of it read-only, from the game server root that is the working directory, normally
 * {@code C:\l2-rust\l2jmobius\dist\game}):
 * <ol>
 * <li>{@code ConfigLoader.init()}: every {@code ./config/*.ini} (the same keys and defaults the server reads).</li>
 * <li>{@code ThreadPool.init()}: the data loaders and some constructors schedule tasks.</li>
 * <li>{@code handlers.EffectMasterHandler.main()}: registers the effect classes (compiled from {@code data/scripts} through the source path), the skill XML parser needs them.</li>
 * <li>The datapack loaders the stat code reads: {@code ExperienceData}, {@code SkillTreeData}, {@code SkillData}, {@code ItemData}, {@code EnchantItemHPBonusData}, {@code ArmorSetData},
 * {@code HennaData}, {@code HitConditionBonusData}, {@code PlayerTemplateData}, {@code NpcData}.</li>
 * </ol>
 * Not started: the real {@code DatabaseFactory} and {@code IdManager} (both replaced by stubs in {@code java-vectors/src}), {@code World.init()}, the script engine, the geodata engine, zones, spawns, quests.
 * <p>
 * The helpers below build a {@link Player} the way {@code Player.restore} does (private constructor by reflection, then level, class, fists weapon, skills, items) but without any SQL.
 */
public final class Bootstrap
{
	private static boolean _done = false;
	private static final AtomicInteger NEXT_OBJECT_ID = new AtomicInteger(0x10000000);

	private Bootstrap()
	{
	}

	/**
	 * Starts the minimum server once. Must be called with the working directory set to the game server root (the folder that holds {@code config/} and {@code data/}).
	 */
	public static synchronized void init() throws Exception
	{
		if (_done)
		{
			return;
		}

		final File config = new File("./config/Server.ini");
		if (!config.isFile() || !new File("./data/stats").isDirectory())
		{
			throw new IllegalStateException("Working directory is not a game server root (needs config/ and data/stats): " + new File(".").getCanonicalPath());
		}

		// The loaders log a lot and Player/BlockList/ContactList log the refused (stubbed) database access; the generators print their own summary.
		Logger.getLogger("").setLevel(Level.WARNING);
		final Filter quiet = new Filter()
		{
			@Override
			public boolean isLoggable(LogRecord record)
			{
				final Throwable t = record.getThrown();
				return (t == null) || (t.getMessage() == null) || !t.getMessage().contains("java-vectors: database access is disabled");
			}
		};
		for (Handler handler : Logger.getLogger("").getHandlers())
		{
			handler.setFilter(quiet);
		}

		ConfigLoader.init();
		ThreadPool.init();

		if (!ServerConfig.DATAPACK_ROOT.equals(new File(".").getCanonicalFile()))
		{
			throw new IllegalStateException("DatapackRoot is not the working directory: " + ServerConfig.DATAPACK_ROOT);
		}

		handlers.EffectMasterHandler.main(new String[0]);

		ExperienceData.getInstance();
		SkillTreeData.getInstance();
		SkillData.getInstance();
		ItemData.getInstance();
		EnchantItemHPBonusData.getInstance();
		ArmorSetData.getInstance();
		HennaData.getInstance();
		HitConditionBonusData.getInstance();
		PlayerTemplateData.getInstance();
		NpcData.getInstance();

		// Everything is loaded. From here on no scheduled task may run: tasks the game code schedules (regeneration, SevenSigns period changes, vitality, ...)
		// would race with the generators and make the output depend on timing. The pools discard tasks once they are shut down.
		ThreadPool.shutdown();

		_done = true;
	}

	/** A fresh object id (ascending from 0x10000000), the same range Java's IdManager starts with. */
	public static int nextObjectId()
	{
		return NEXT_OBJECT_ID.getAndIncrement();
	}

	/**
	 * Builds a player like {@code Player.restore} does, minus the SQL: private constructor, class, level, exp, fists weapon. No items, no skills, no henna.
	 * @param classId the class id (must have a template)
	 * @param female the appearance sex
	 * @param level the character level
	 * @return the player, or {@code null} when the class has no template
	 */
	public static Player newPlayer(int classId, boolean female, int level) throws Exception
	{
		final PlayerTemplate template = PlayerTemplateData.getInstance().getTemplate(classId);
		if (template == null)
		{
			return null;
		}

		final Constructor<Player> constructor = Player.class.getDeclaredConstructor(int.class, PlayerTemplate.class, String.class, PlayerAppearance.class);
		constructor.setAccessible(true);
		final Player player = constructor.newInstance(nextObjectId(), template, "vectors", new PlayerAppearance((byte) 0, (byte) 0, (byte) 0, female));

		// Player.restore: stat.setExp, stat.setLevel, setFistsWeaponItem, setBaseClass, _activeClass.
		player.getStat().setExp(ExperienceData.getInstance().getExpForLevel(level));
		player.getStat().setLevel((byte) level);
		player.setFistsWeaponItem(player.findFistsWeaponItem(classId));
		player.setBaseClass(classId);
		final Field active = Player.class.getDeclaredField("_activeClass");
		active.setAccessible(true);
		active.setInt(player, classId);
		return player;
	}

	/**
	 * Gives the player what {@code Player.rewardSkills()} gives with the shipped {@code Player.ini}, minus the database writes ({@code addSkill(skill, true)} and {@code storeSkills} would insert rows).
	 * @return the skills in the order they were added, as {@code [id, level]}
	 */
	public static List<int[]> giveSkills(Player player)
	{
		final List<int[]> added = new ArrayList<>();
		if (org.l2jmobius.gameserver.config.PlayerConfig.AUTO_LEARN_SKILLS)
		{
			// Player.giveAvailableSkills(AUTO_LEARN_FS_SKILLS, true, AUTO_LEARN_SKILLS_WITHOUT_ITEMS)
			for (Skill skill : SkillTreeData.getInstance().getAllAvailableSkills(player, player.getPlayerClass(), org.l2jmobius.gameserver.config.PlayerConfig.AUTO_LEARN_FS_SKILLS, true, org.l2jmobius.gameserver.config.PlayerConfig.AUTO_LEARN_SKILLS_WITHOUT_ITEMS))
			{
				if (player.getKnownSkill(skill.getId()) == skill)
				{
					continue;
				}

				player.addSkill(skill, false);
				added.add(new int[]
				{
					skill.getId(),
					skill.getLevel()
				});
			}
		}
		else
		{
			// Player.giveAvailableAutoGetSkills()
			for (SkillLearn learn : SkillTreeData.getInstance().getAvailableAutoGetSkills(player))
			{
				final Skill skill = SkillData.getInstance().getSkill(learn.getSkillId(), learn.getSkillLevel());
				if (skill != null)
				{
					player.addSkill(skill, false);
					added.add(new int[]
					{
						skill.getId(),
						skill.getLevel()
					});
				}
			}
		}

		return added;
	}

	/**
	 * Puts an item into the player's inventory list and equips it through the real {@code Inventory.equipItem} (all paperdoll listeners run: stat funcs, armor sets, item skills).
	 * The item has no owner id, so {@code Item.updateDatabase} returns early and nothing is written.
	 * @return the item
	 */
	public static Item equip(Player player, int itemId, int enchant) throws Exception
	{
		final Item item = new Item(nextObjectId(), itemId);
		item.setEnchantLevel(enchant);
		final Method add = Inventory.class.getDeclaredMethod("addItem", Item.class);
		add.setAccessible(true);
		add.invoke(player.getInventory(), item);
		player.getInventory().equipItem(item);
		return item;
	}

	/**
	 * Builds an NPC with the real class named by its template type, like {@code Spawn} does ({@code entity.actor.instance.<type>}).
	 * @return the npc, or {@code null} when there is no template
	 */
	public static Npc newNpc(int npcId) throws Exception
	{
		final NpcTemplate template = NpcData.getInstance().getTemplate(npcId);
		if (template == null)
		{
			return null;
		}

		final Constructor<? extends Npc> constructor = Class.forName("org.l2jmobius.gameserver.entity.actor.instance." + template.getType()).asSubclass(Npc.class).getConstructor(NpcTemplate.class);
		final Npc npc = constructor.newInstance(template);
		return npc;
	}

	/** @return the fists weapon check used by the generators to print the weapon name */
	public static String weaponName(Weapon weapon)
	{
		return weapon == null ? null : weapon.getName();
	}
}
