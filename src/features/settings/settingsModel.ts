import source from './defaults.json';
export type HomeBackground='physics-1'|'physics-2'|'physics-3';
export type HomeBackgroundMediaType='builtin-image'|'custom-image'|'custom-animated-image'|'custom-video';
export type HomeBackgroundBuiltinId='default'|'style1'|'style2';
export type HomeCompositionId='default'|'style1'|'style2';
export type HomeWordmarkStyle='default'|'style12';
export type ColorTheme='scientific-blue'|'indigo'|'teal'|'violet'|'amber'|'slate';
export type AppSettings=Omit<typeof source.preferences,'home.background_media_type'|'home.background_builtin_id'|'home.wordmark_style'|'home.composition_profile'|'home.greeting_mode'|'home.background_builtin'|'appearance.color_theme'|'appearance.density'>&{
 'home.background_media_type':HomeBackgroundMediaType;'home.background_builtin_id':HomeBackgroundBuiltinId;'home.wordmark_style':HomeWordmarkStyle;'home.composition_profile':HomeCompositionId;'home.greeting_mode':'automatic'|'custom';'home.background_builtin':HomeBackground;'appearance.color_theme':ColorTheme;'appearance.density':'comfortable'|'compact';
};
export type SettingKey=keyof AppSettings;
export const settingsDefaults=source.preferences as AppSettings;
export const numberBounds:Partial<Record<SettingKey,readonly [number,number]>>={'appearance.page_surface_alpha':[0,1],'projects.name_font_size':[14,28],'home.subtitle_font_size':[16,56],'startup.default_width':[1050,3840],'startup.default_height':[680,2160]};
export function validateSetting<K extends SettingKey>(key:K,value:AppSettings[K]){
 if(!(key in settingsDefaults)||typeof value!==typeof settingsDefaults[key])throw new Error('设置值类型无效');
 const bounds=numberBounds[key];if(bounds&&(typeof value!=='number'||!Number.isFinite(value)||value<bounds[0]||value>bounds[1]))throw new Error('设置值超出允许范围');
 const limits:Partial<Record<SettingKey,number>>={'home.message':120,'home.custom_greeting':40,'home.subtitle_text':40,'home.subtitle_font_family':120};
 if(typeof value==='string'&&value.length>(limits[key]??4096))throw new Error('设置内容过长');
 const options:Partial<Record<SettingKey,readonly string[]>>={'home.greeting_mode':['automatic','custom'],'home.composition_profile':['default','style1','style2'],'home.wordmark_style':['default','style12'],'home.background_builtin_id':['default','style1','style2'],'appearance.color_theme':['scientific-blue','indigo','teal','violet','amber','slate'],'appearance.density':['comfortable','compact']};
 if(options[key]&&!options[key]!.includes(String(value)))throw new Error('设置选项无效');
}
